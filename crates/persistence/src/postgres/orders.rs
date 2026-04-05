use domain::order::{
    OrderLifecycleContractError, OrderLifecycleReasonCode, OrderLifecycleState,
    OrderLifecycleValidationIssue, OrderMode, OrderRecord, OrderStateTransition,
    normalize_order_cancel_idempotency_key, normalize_order_idempotency_key,
    normalize_order_submission_idempotency_key, validate_order_record,
    validate_order_state_transition, validate_order_transition,
};
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_ORDER_SQL: &str = r#"
    INSERT INTO orders (
        order_id,
        market_id,
        order_mode,
        lifecycle_state,
        submission_idempotency_key,
        cancel_idempotency_key,
        last_transition_sequence,
        last_reason_code,
        correlation_id,
        created_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10::timestamptz, $11::timestamptz
    )
"#;

const UPDATE_ORDER_SQL: &str = r#"
    UPDATE orders
    SET
        lifecycle_state = $2,
        cancel_idempotency_key = $3,
        last_transition_sequence = $4,
        last_reason_code = $5,
        correlation_id = $6,
        updated_at_utc = $7::timestamptz
    WHERE order_id = $1
"#;

const INSERT_ORDER_STATE_TRANSITION_SQL: &str = r#"
    INSERT INTO order_state_transitions (
        transition_id,
        order_id,
        market_id,
        order_mode,
        from_state,
        to_state,
        reason_code,
        idempotency_key,
        correlation_id,
        transition_sequence,
        transitioned_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz
    )
"#;

const LOAD_ORDER_FOR_UPDATE_SQL: &str = r#"
    SELECT
        order_id,
        market_id,
        order_mode,
        lifecycle_state,
        submission_idempotency_key,
        cancel_idempotency_key,
        last_transition_sequence,
        last_reason_code,
        correlation_id,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM orders
    WHERE order_id = $1
    FOR UPDATE
"#;

const LOAD_ORDER_SQL: &str = r#"
    SELECT
        order_id,
        market_id,
        order_mode,
        lifecycle_state,
        submission_idempotency_key,
        cancel_idempotency_key,
        last_transition_sequence,
        last_reason_code,
        correlation_id,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM orders
    WHERE order_id = $1
"#;

const LOAD_ORDER_TRANSITIONS_SQL: &str = r#"
    SELECT
        transition_id,
        order_id,
        market_id,
        order_mode,
        from_state,
        to_state,
        reason_code,
        idempotency_key,
        correlation_id,
        transition_sequence,
        to_char(transitioned_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS transitioned_at_utc
    FROM order_state_transitions
    WHERE order_id = $1
    ORDER BY transition_sequence ASC
"#;

const LOAD_TRANSITION_BY_IDEMPOTENCY_KEY_SQL: &str = r#"
    SELECT
        transition_id,
        order_id,
        market_id,
        order_mode,
        from_state,
        to_state,
        reason_code,
        idempotency_key,
        correlation_id,
        transition_sequence,
        to_char(transitioned_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS transitioned_at_utc
    FROM order_state_transitions
    WHERE order_id = $1
      AND idempotency_key = $2
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLifecyclePersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<OrderLifecycleValidationIssue>,
}

impl OrderLifecyclePersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<OrderLifecycleValidationIssue>,
    ) -> Self {
        Self {
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn persistence_unavailable(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: OrderLifecycleReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: OrderLifecycleReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: OrderLifecycleReasonCode::PersistenceUnavailable.code(),
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for OrderLifecyclePersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for OrderLifecyclePersistenceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderLifecyclePersistDisposition {
    Applied,
    Duplicate,
    Rejected,
    AlreadyTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLifecyclePersistOutcome {
    pub disposition: OrderLifecyclePersistDisposition,
    pub reason_code: String,
    pub order: OrderRecord,
    pub transition: Option<OrderStateTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLifecycleTransitionCommand {
    pub order_id: String,
    pub market_id: String,
    pub mode: OrderMode,
    pub target_state: OrderLifecycleState,
    pub reason_code: String,
    pub correlation_id: String,
    pub idempotency_key: String,
    pub transitioned_at_utc: String,
}

pub async fn persist_order_transition(
    pool: &PgPool,
    command: &OrderLifecycleTransitionCommand,
) -> Result<OrderLifecyclePersistOutcome, OrderLifecyclePersistenceError> {
    validate_transition_command(command)?;
    let normalized_idempotency_key = normalize_order_idempotency_key(&command.idempotency_key);
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| classify_query_error("begin_order_transition_tx", error))?;

    let current_order = sqlx::query(LOAD_ORDER_FOR_UPDATE_SQL)
        .bind(command.order_id.trim())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| classify_query_error("load_order_for_update", error))?
        .map(decode_order_row)
        .transpose()?;

    let duplicate_transition = sqlx::query(LOAD_TRANSITION_BY_IDEMPOTENCY_KEY_SQL)
        .bind(command.order_id.trim())
        .bind(&normalized_idempotency_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| classify_query_error("load_transition_by_idempotency_key", error))?
        .map(decode_transition_row)
        .transpose()?;

    if duplicate_transition.is_some() {
        tx.commit()
            .await
            .map_err(|error| classify_query_error("commit_duplicate_order_transition_tx", error))?;
        let order = match current_order {
            Some(order) => order,
            None => {
                return Err(OrderLifecyclePersistenceError::invalid_payload(
                    format!(
                        "duplicate transition found for missing order `{}`",
                        command.order_id
                    ),
                    Vec::new(),
                ));
            }
        };
        return Ok(OrderLifecyclePersistOutcome {
            disposition: OrderLifecyclePersistDisposition::Duplicate,
            reason_code: OrderLifecycleReasonCode::DuplicateIdempotencyKey
                .code()
                .to_string(),
            order,
            transition: None,
        });
    }

    let (next_order, transition, disposition) = match current_order {
        Some(current_order) => {
            if current_order.state.is_terminal() {
                tx.commit().await.map_err(|error| {
                    classify_query_error("commit_already_terminal_order_transition_tx", error)
                })?;
                return Ok(OrderLifecyclePersistOutcome {
                    disposition: OrderLifecyclePersistDisposition::AlreadyTerminal,
                    reason_code: OrderLifecycleReasonCode::AlreadyTerminal.code().to_string(),
                    order: current_order,
                    transition: None,
                });
            }
            if validate_order_transition(Some(current_order.state), command.target_state).is_err() {
                tx.commit().await.map_err(|error| {
                    classify_query_error("commit_rejected_order_transition_tx", error)
                })?;
                return Ok(OrderLifecyclePersistOutcome {
                    disposition: OrderLifecyclePersistDisposition::Rejected,
                    reason_code: OrderLifecycleReasonCode::TransitionRejected
                        .code()
                        .to_string(),
                    order: current_order,
                    transition: None,
                });
            }

            let (next_order, transition) =
                project_follow_up_transition(&current_order, command, &normalized_idempotency_key)?;
            (
                next_order,
                transition,
                OrderLifecyclePersistDisposition::Applied,
            )
        }
        None => {
            let (next_order, transition) =
                project_initial_transition(command, &normalized_idempotency_key)?;
            (
                next_order,
                transition,
                OrderLifecyclePersistDisposition::Applied,
            )
        }
    };

    match disposition {
        OrderLifecyclePersistDisposition::Applied => {
            if transition.transition_sequence == 1 {
                sqlx::query(INSERT_ORDER_SQL)
                    .bind(&next_order.order_id)
                    .bind(&next_order.market_id)
                    .bind(next_order.mode.as_str())
                    .bind(next_order.state.as_str())
                    .bind(&next_order.submission_idempotency_key)
                    .bind(&next_order.cancel_idempotency_key)
                    .bind(next_order.last_transition_sequence)
                    .bind(&next_order.last_reason_code)
                    .bind(&next_order.correlation_id)
                    .bind(&next_order.created_at_utc)
                    .bind(&next_order.updated_at_utc)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| classify_query_error("insert_order_projection", error))?;

                sqlx::query(INSERT_ORDER_STATE_TRANSITION_SQL)
                    .bind(&transition.transition_id)
                    .bind(&transition.order_id)
                    .bind(&transition.market_id)
                    .bind(transition.mode.as_str())
                    .bind(transition.from_state.map(OrderLifecycleState::as_str))
                    .bind(transition.to_state.as_str())
                    .bind(&transition.reason_code)
                    .bind(&transition.idempotency_key)
                    .bind(&transition.correlation_id)
                    .bind(transition.transition_sequence)
                    .bind(&transition.transitioned_at_utc)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| {
                        classify_query_error("insert_order_state_transition", error)
                    })?;
            } else {
                sqlx::query(INSERT_ORDER_STATE_TRANSITION_SQL)
                    .bind(&transition.transition_id)
                    .bind(&transition.order_id)
                    .bind(&transition.market_id)
                    .bind(transition.mode.as_str())
                    .bind(transition.from_state.map(OrderLifecycleState::as_str))
                    .bind(transition.to_state.as_str())
                    .bind(&transition.reason_code)
                    .bind(&transition.idempotency_key)
                    .bind(&transition.correlation_id)
                    .bind(transition.transition_sequence)
                    .bind(&transition.transitioned_at_utc)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| {
                        classify_query_error("insert_order_state_transition", error)
                    })?;

                sqlx::query(UPDATE_ORDER_SQL)
                    .bind(&next_order.order_id)
                    .bind(next_order.state.as_str())
                    .bind(&next_order.cancel_idempotency_key)
                    .bind(next_order.last_transition_sequence)
                    .bind(&next_order.last_reason_code)
                    .bind(&next_order.correlation_id)
                    .bind(&next_order.updated_at_utc)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| classify_query_error("update_order_projection", error))?;
            }
        }
        OrderLifecyclePersistDisposition::Duplicate
        | OrderLifecyclePersistDisposition::Rejected
        | OrderLifecyclePersistDisposition::AlreadyTerminal => {}
    }

    tx.commit()
        .await
        .map_err(|error| classify_query_error("commit_order_transition_tx", error))?;

    Ok(OrderLifecyclePersistOutcome {
        disposition,
        reason_code: next_order.last_reason_code.clone(),
        order: next_order,
        transition: Some(transition),
    })
}

pub async fn load_order(
    pool: &PgPool,
    order_id: &str,
) -> Result<Option<OrderRecord>, OrderLifecyclePersistenceError> {
    validate_non_empty("order_id", order_id)?;
    let row = sqlx::query(LOAD_ORDER_SQL)
        .bind(order_id.trim())
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_order", error))?;
    row.map(decode_order_row).transpose()
}

pub async fn load_order_state_transitions(
    pool: &PgPool,
    order_id: &str,
) -> Result<Vec<OrderStateTransition>, OrderLifecyclePersistenceError> {
    validate_non_empty("order_id", order_id)?;
    let rows = sqlx::query(LOAD_ORDER_TRANSITIONS_SQL)
        .bind(order_id.trim())
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_order_state_transitions", error))?;
    rows.into_iter().map(decode_transition_row).collect()
}

fn project_initial_transition(
    command: &OrderLifecycleTransitionCommand,
    normalized_idempotency_key: &str,
) -> Result<(OrderRecord, OrderStateTransition), OrderLifecyclePersistenceError> {
    validate_order_transition(None, command.target_state).map_err(map_contract_error)?;

    let submission_idempotency_key =
        normalize_order_submission_idempotency_key(&command.order_id, normalized_idempotency_key);
    let cancel_idempotency_key = if command.target_state == OrderLifecycleState::Canceled {
        Some(normalize_order_cancel_idempotency_key(
            &command.order_id,
            normalized_idempotency_key,
        ))
    } else {
        None
    };

    let order = OrderRecord {
        order_id: command.order_id.trim().to_string(),
        market_id: command.market_id.trim().to_string(),
        mode: command.mode,
        state: command.target_state,
        submission_idempotency_key,
        cancel_idempotency_key,
        last_transition_sequence: 1,
        last_reason_code: command.reason_code.clone(),
        correlation_id: command.correlation_id.trim().to_string(),
        created_at_utc: command.transitioned_at_utc.trim().to_string(),
        updated_at_utc: command.transitioned_at_utc.trim().to_string(),
    };
    validate_order_record(&order).map_err(map_contract_error)?;

    let transition = OrderStateTransition {
        transition_id: transition_id(&command.order_id, 1, &command.transitioned_at_utc),
        order_id: command.order_id.trim().to_string(),
        market_id: command.market_id.trim().to_string(),
        mode: command.mode,
        from_state: None,
        to_state: command.target_state,
        reason_code: command.reason_code.clone(),
        idempotency_key: normalized_idempotency_key.to_string(),
        correlation_id: command.correlation_id.trim().to_string(),
        transition_sequence: 1,
        transitioned_at_utc: command.transitioned_at_utc.trim().to_string(),
    };
    validate_order_state_transition(&transition).map_err(map_contract_error)?;
    Ok((order, transition))
}

fn project_follow_up_transition(
    current_order: &OrderRecord,
    command: &OrderLifecycleTransitionCommand,
    normalized_idempotency_key: &str,
) -> Result<(OrderRecord, OrderStateTransition), OrderLifecyclePersistenceError> {
    if current_order.market_id != command.market_id.trim() {
        return Err(OrderLifecyclePersistenceError::invalid_payload(
            "market_id does not match existing order projection",
            Vec::new(),
        ));
    }
    if current_order.mode != command.mode {
        return Err(OrderLifecyclePersistenceError::invalid_payload(
            "order_mode does not match existing order projection",
            Vec::new(),
        ));
    }
    validate_order_transition(Some(current_order.state), command.target_state)
        .map_err(map_contract_error)?;

    let next_sequence = current_order.last_transition_sequence + 1;
    let mut cancel_idempotency_key = current_order.cancel_idempotency_key.clone();
    if command.target_state == OrderLifecycleState::Canceled {
        cancel_idempotency_key = Some(normalize_order_cancel_idempotency_key(
            &command.order_id,
            normalized_idempotency_key,
        ));
    }

    let next_order = OrderRecord {
        order_id: current_order.order_id.clone(),
        market_id: current_order.market_id.clone(),
        mode: current_order.mode,
        state: command.target_state,
        submission_idempotency_key: current_order.submission_idempotency_key.clone(),
        cancel_idempotency_key,
        last_transition_sequence: next_sequence,
        last_reason_code: command.reason_code.clone(),
        correlation_id: command.correlation_id.trim().to_string(),
        created_at_utc: current_order.created_at_utc.clone(),
        updated_at_utc: command.transitioned_at_utc.trim().to_string(),
    };
    validate_order_record(&next_order).map_err(map_contract_error)?;

    let transition = OrderStateTransition {
        transition_id: transition_id(
            &command.order_id,
            next_sequence,
            &command.transitioned_at_utc,
        ),
        order_id: command.order_id.trim().to_string(),
        market_id: command.market_id.trim().to_string(),
        mode: command.mode,
        from_state: Some(current_order.state),
        to_state: command.target_state,
        reason_code: command.reason_code.clone(),
        idempotency_key: normalized_idempotency_key.to_string(),
        correlation_id: command.correlation_id.trim().to_string(),
        transition_sequence: next_sequence,
        transitioned_at_utc: command.transitioned_at_utc.trim().to_string(),
    };
    validate_order_state_transition(&transition).map_err(map_contract_error)?;
    Ok((next_order, transition))
}

fn decode_order_row(
    row: sqlx::postgres::PgRow,
) -> Result<OrderRecord, OrderLifecyclePersistenceError> {
    let mode: String = row
        .try_get("order_mode")
        .map_err(|error| OrderLifecyclePersistenceError::row_decode_failure("order_mode", error))?;
    let mode = OrderMode::parse(&mode).map_err(map_contract_error)?;
    let state: String = row.try_get("lifecycle_state").map_err(|error| {
        OrderLifecyclePersistenceError::row_decode_failure("lifecycle_state", error)
    })?;
    let state = OrderLifecycleState::parse(&state).map_err(map_contract_error)?;

    let order = OrderRecord {
        order_id: row.try_get("order_id").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("order_id", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("market_id", error)
        })?,
        mode,
        state,
        submission_idempotency_key: row.try_get("submission_idempotency_key").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("submission_idempotency_key", error)
        })?,
        cancel_idempotency_key: row.try_get("cancel_idempotency_key").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("cancel_idempotency_key", error)
        })?,
        last_transition_sequence: row.try_get("last_transition_sequence").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("last_transition_sequence", error)
        })?,
        last_reason_code: row.try_get("last_reason_code").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("last_reason_code", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("correlation_id", error)
        })?,
        created_at_utc: row.try_get("created_at_utc").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("created_at_utc", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };
    validate_order_record(&order).map_err(map_contract_error)?;
    Ok(order)
}

fn decode_transition_row(
    row: sqlx::postgres::PgRow,
) -> Result<OrderStateTransition, OrderLifecyclePersistenceError> {
    let mode: String = row
        .try_get("order_mode")
        .map_err(|error| OrderLifecyclePersistenceError::row_decode_failure("order_mode", error))?;
    let mode = OrderMode::parse(&mode).map_err(map_contract_error)?;
    let to_state: String = row
        .try_get("to_state")
        .map_err(|error| OrderLifecyclePersistenceError::row_decode_failure("to_state", error))?;
    let to_state = OrderLifecycleState::parse(&to_state).map_err(map_contract_error)?;
    let from_state: Option<String> = row
        .try_get("from_state")
        .map_err(|error| OrderLifecyclePersistenceError::row_decode_failure("from_state", error))?;
    let from_state = match from_state {
        Some(value) => Some(OrderLifecycleState::parse(&value).map_err(map_contract_error)?),
        None => None,
    };

    let transition = OrderStateTransition {
        transition_id: row.try_get("transition_id").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("transition_id", error)
        })?,
        order_id: row.try_get("order_id").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("order_id", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("market_id", error)
        })?,
        mode,
        from_state,
        to_state,
        reason_code: row.try_get("reason_code").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("reason_code", error)
        })?,
        idempotency_key: row.try_get("idempotency_key").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("idempotency_key", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("correlation_id", error)
        })?,
        transition_sequence: row.try_get("transition_sequence").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("transition_sequence", error)
        })?,
        transitioned_at_utc: row.try_get("transitioned_at_utc").map_err(|error| {
            OrderLifecyclePersistenceError::row_decode_failure("transitioned_at_utc", error)
        })?,
    };
    validate_order_state_transition(&transition).map_err(map_contract_error)?;
    Ok(transition)
}

fn validate_transition_command(
    command: &OrderLifecycleTransitionCommand,
) -> Result<(), OrderLifecyclePersistenceError> {
    validate_non_empty("order_id", &command.order_id)?;
    validate_non_empty("market_id", &command.market_id)?;
    validate_non_empty("reason_code", &command.reason_code)?;
    validate_non_empty("correlation_id", &command.correlation_id)?;
    validate_non_empty("idempotency_key", &command.idempotency_key)?;
    validate_non_empty("transitioned_at_utc", &command.transitioned_at_utc)?;
    OrderLifecycleReasonCode::parse(&command.reason_code).map_err(map_contract_error)?;
    if normalize_order_idempotency_key(&command.idempotency_key) != command.idempotency_key {
        return Err(OrderLifecyclePersistenceError::invalid_payload(
            "idempotency_key must be normalized (trimmed lowercase)",
            Vec::new(),
        ));
    }
    let transition = OrderStateTransition {
        transition_id: transition_id(&command.order_id, 1, &command.transitioned_at_utc),
        order_id: command.order_id.trim().to_string(),
        market_id: command.market_id.trim().to_string(),
        mode: command.mode,
        from_state: None,
        to_state: command.target_state,
        reason_code: command.reason_code.clone(),
        idempotency_key: command.idempotency_key.clone(),
        correlation_id: command.correlation_id.trim().to_string(),
        transition_sequence: 1,
        transitioned_at_utc: command.transitioned_at_utc.trim().to_string(),
    };
    validate_order_state_transition(&transition).map_err(map_contract_error)?;
    Ok(())
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), OrderLifecyclePersistenceError> {
    if value.trim().is_empty() {
        return Err(OrderLifecyclePersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            Vec::new(),
        ));
    }
    Ok(())
}

fn transition_id(order_id: &str, sequence: i64, timestamp_utc: &str) -> String {
    let compact_timestamp: String = timestamp_utc
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    format!(
        "transition::{}::{}::{}",
        order_id.trim(),
        sequence,
        compact_timestamp
    )
}

fn map_contract_error(error: OrderLifecycleContractError) -> OrderLifecyclePersistenceError {
    OrderLifecyclePersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> OrderLifecyclePersistenceError {
    if is_constraint_error(&error) {
        return OrderLifecyclePersistenceError::constraint_violation(operation, error);
    }
    OrderLifecyclePersistenceError::persistence_unavailable(operation, error)
}

fn is_constraint_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => database_error
            .code()
            .map(|code| code.starts_with("23") || code == "55000")
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER_LIFECYCLE_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406050000_order_lifecycle.sql");

    fn sample_transition_command(
        target_state: OrderLifecycleState,
        reason_code: OrderLifecycleReasonCode,
    ) -> OrderLifecycleTransitionCommand {
        OrderLifecycleTransitionCommand {
            order_id: "order-1".to_string(),
            market_id: "market-1".to_string(),
            mode: OrderMode::Limit,
            target_state,
            reason_code: reason_code.code().to_string(),
            correlation_id: "corr-order-1".to_string(),
            idempotency_key: "submit::order-1".to_string(),
            transitioned_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_existing_order() -> OrderRecord {
        OrderRecord {
            order_id: "order-1".to_string(),
            market_id: "market-1".to_string(),
            mode: OrderMode::Limit,
            state: OrderLifecycleState::Live,
            submission_idempotency_key: "submit::order-1".to_string(),
            cancel_idempotency_key: None,
            last_transition_sequence: 1,
            last_reason_code: OrderLifecycleReasonCode::SubmissionAccepted
                .code()
                .to_string(),
            correlation_id: "corr-order-1".to_string(),
            created_at_utc: "2026-04-06T00:00:00Z".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_order_lifecycle_schema_scope() {
        assert!(ORDER_LIFECYCLE_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS orders"));
        assert!(
            ORDER_LIFECYCLE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS order_state_transitions")
        );
        assert!(
            !ORDER_LIFECYCLE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS order_event_offsets")
        );
        assert!(
            !ORDER_LIFECYCLE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS user_stream_events")
        );
        assert!(
            !ORDER_LIFECYCLE_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS freshness_gate_events")
        );
    }

    #[test]
    fn migration_enforces_constraints_indexes_and_sequence_monotonicity() {
        assert!(ORDER_LIFECYCLE_MIGRATION_SQL.contains("order_mode IN ('limit', 'reduce_only')"));
        assert!(ORDER_LIFECYCLE_MIGRATION_SQL.contains("lifecycle_state IN ("));
        assert!(ORDER_LIFECYCLE_MIGRATION_SQL.contains("UNIQUE (order_id, transition_sequence)"));
        assert!(ORDER_LIFECYCLE_MIGRATION_SQL.contains("UNIQUE (order_id, idempotency_key)"));
        assert!(ORDER_LIFECYCLE_MIGRATION_SQL.contains("idx_orders_market_state_updated"));
        assert!(ORDER_LIFECYCLE_MIGRATION_SQL.contains("idx_order_state_transitions_order_replay"));
        assert!(
            ORDER_LIFECYCLE_MIGRATION_SQL.contains("idx_order_state_transitions_correlation_time")
        );
        assert!(
            ORDER_LIFECYCLE_MIGRATION_SQL
                .contains("enforce_order_transition_sequence_monotonicity")
        );
        assert!(
            ORDER_LIFECYCLE_MIGRATION_SQL
                .contains("trg_order_state_transitions_sequence_monotonicity")
        );
    }

    #[test]
    fn transition_command_validation_rejects_non_normalized_idempotency_key() {
        let mut command = sample_transition_command(
            OrderLifecycleState::Pending,
            OrderLifecycleReasonCode::SubmissionAccepted,
        );
        command.idempotency_key = " Submit::Order-1 ".to_string();
        let error = validate_transition_command(&command)
            .expect_err("non-normalized idempotency key should fail validation");
        assert_eq!(error.code, OrderLifecycleReasonCode::InvalidPayload.code());
    }

    #[test]
    fn projection_rejects_terminal_to_non_terminal_regression() {
        let mut order = sample_existing_order();
        order.state = OrderLifecycleState::Filled;
        let command = sample_transition_command(
            OrderLifecycleState::Live,
            OrderLifecycleReasonCode::VenueUpdateAccepted,
        );
        let error = project_follow_up_transition(&order, &command, "venue::order-1::2")
            .expect_err("terminal-to-non-terminal transition must fail");
        assert_eq!(error.code, OrderLifecycleReasonCode::AlreadyTerminal.code());
    }

    #[test]
    fn projection_appends_transition_and_updates_order_deterministically() {
        let order = sample_existing_order();
        let command = sample_transition_command(
            OrderLifecycleState::PartiallyFilled,
            OrderLifecycleReasonCode::VenueUpdateAccepted,
        );
        let (next_order, transition) =
            project_follow_up_transition(&order, &command, "venue::order-1::2")
                .expect("follow-up transition should project");

        assert_eq!(next_order.state, OrderLifecycleState::PartiallyFilled);
        assert_eq!(next_order.last_transition_sequence, 2);
        assert_eq!(
            next_order.last_reason_code,
            OrderLifecycleReasonCode::VenueUpdateAccepted.code()
        );
        assert_eq!(transition.transition_sequence, 2);
        assert_eq!(transition.from_state, Some(OrderLifecycleState::Live));
        assert_eq!(transition.to_state, OrderLifecycleState::PartiallyFilled);
        assert_eq!(transition.idempotency_key, "venue::order-1::2");
    }
}
