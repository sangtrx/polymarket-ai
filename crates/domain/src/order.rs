use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderKey {
    pub order_id: String,
    pub market_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderMode {
    Limit,
    ReduceOnly,
}

impl OrderMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Limit => "limit",
            Self::ReduceOnly => "reduce_only",
        }
    }

    pub fn parse(value: &str) -> Result<Self, OrderLifecycleContractError> {
        match value {
            "limit" => Ok(Self::Limit),
            "reduce_only" => Ok(Self::ReduceOnly),
            _ => Err(OrderLifecycleContractError::invalid_payload(format!(
                "unknown order mode `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderLifecycleState {
    Pending,
    Live,
    PartiallyFilled,
    Filled,
    Canceled,
    Expired,
}

impl OrderLifecycleState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Live => "live",
            Self::PartiallyFilled => "partially_filled",
            Self::Filled => "filled",
            Self::Canceled => "canceled",
            Self::Expired => "expired",
        }
    }

    pub fn parse(value: &str) -> Result<Self, OrderLifecycleContractError> {
        match value {
            "pending" => Ok(Self::Pending),
            "live" => Ok(Self::Live),
            "partially_filled" => Ok(Self::PartiallyFilled),
            "filled" => Ok(Self::Filled),
            "canceled" => Ok(Self::Canceled),
            "expired" => Ok(Self::Expired),
            _ => Err(OrderLifecycleContractError::invalid_payload(format!(
                "unknown order lifecycle state `{value}`"
            ))),
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Filled | Self::Canceled | Self::Expired)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderLifecycleReasonCode {
    SubmissionAccepted,
    CancelAccepted,
    BatchCancelAccepted,
    VenueUpdateAccepted,
    TransitionRejected,
    UnsupportedVenueState,
    AlreadyTerminal,
    DuplicateIdempotencyKey,
    RetryableFailure,
    HardFailure,
    PersistenceUnavailable,
    VenueUnavailable,
    Unauthorized,
    AuthExpired,
    InvalidPayload,
}

impl OrderLifecycleReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SubmissionAccepted => "order_lifecycle_submission_accepted",
            Self::CancelAccepted => "order_lifecycle_cancel_accepted",
            Self::BatchCancelAccepted => "order_lifecycle_batch_cancel_accepted",
            Self::VenueUpdateAccepted => "order_lifecycle_venue_update_accepted",
            Self::TransitionRejected => "order_lifecycle_transition_rejected",
            Self::UnsupportedVenueState => "order_lifecycle_unsupported_venue_state",
            Self::AlreadyTerminal => "order_lifecycle_already_terminal",
            Self::DuplicateIdempotencyKey => "order_lifecycle_duplicate_idempotency_key",
            Self::RetryableFailure => "order_lifecycle_retryable_failure",
            Self::HardFailure => "order_lifecycle_hard_failure",
            Self::PersistenceUnavailable => "order_lifecycle_persistence_unavailable",
            Self::VenueUnavailable => "order_lifecycle_venue_unavailable",
            Self::Unauthorized => "order_lifecycle_unauthorized",
            Self::AuthExpired => "order_lifecycle_auth_expired",
            Self::InvalidPayload => "order_lifecycle_invalid_payload",
        }
    }

    pub fn parse(value: &str) -> Result<Self, OrderLifecycleContractError> {
        match value {
            "order_lifecycle_submission_accepted" => Ok(Self::SubmissionAccepted),
            "order_lifecycle_cancel_accepted" => Ok(Self::CancelAccepted),
            "order_lifecycle_batch_cancel_accepted" => Ok(Self::BatchCancelAccepted),
            "order_lifecycle_venue_update_accepted" => Ok(Self::VenueUpdateAccepted),
            "order_lifecycle_transition_rejected" => Ok(Self::TransitionRejected),
            "order_lifecycle_unsupported_venue_state" => Ok(Self::UnsupportedVenueState),
            "order_lifecycle_already_terminal" => Ok(Self::AlreadyTerminal),
            "order_lifecycle_duplicate_idempotency_key" => Ok(Self::DuplicateIdempotencyKey),
            "order_lifecycle_retryable_failure" => Ok(Self::RetryableFailure),
            "order_lifecycle_hard_failure" => Ok(Self::HardFailure),
            "order_lifecycle_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "order_lifecycle_venue_unavailable" => Ok(Self::VenueUnavailable),
            "order_lifecycle_unauthorized" => Ok(Self::Unauthorized),
            "order_lifecycle_auth_expired" => Ok(Self::AuthExpired),
            "order_lifecycle_invalid_payload" => Ok(Self::InvalidPayload),
            _ => Err(OrderLifecycleContractError::invalid_payload(format!(
                "unknown order lifecycle reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderLifecycleValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderLifecycleContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<OrderLifecycleValidationIssue>,
}

impl OrderLifecycleContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<OrderLifecycleValidationIssue>,
    ) -> Self {
        Self {
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn already_terminal(message: impl Into<String>) -> Self {
        Self {
            code: OrderLifecycleReasonCode::AlreadyTerminal.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderRecord {
    pub order_id: String,
    pub market_id: String,
    pub mode: OrderMode,
    pub state: OrderLifecycleState,
    pub submission_idempotency_key: String,
    pub cancel_idempotency_key: Option<String>,
    pub last_transition_sequence: i64,
    pub last_reason_code: String,
    pub correlation_id: String,
    pub created_at_utc: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderStateTransition {
    pub transition_id: String,
    pub order_id: String,
    pub market_id: String,
    pub mode: OrderMode,
    pub from_state: Option<OrderLifecycleState>,
    pub to_state: OrderLifecycleState,
    pub reason_code: String,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub transition_sequence: i64,
    pub transitioned_at_utc: String,
}

pub fn normalize_order_idempotency_key(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn normalize_order_submission_idempotency_key(order_id: &str, raw: &str) -> String {
    let normalized = normalize_order_idempotency_key(raw);
    if normalized.is_empty() {
        return format!(
            "submit::{}",
            normalize_order_idempotency_key(order_id.trim())
        );
    }
    normalized
}

pub fn normalize_order_cancel_idempotency_key(order_id: &str, raw: &str) -> String {
    let normalized = normalize_order_idempotency_key(raw);
    if normalized.is_empty() {
        return format!(
            "cancel::{}",
            normalize_order_idempotency_key(order_id.trim())
        );
    }
    normalized
}

pub fn normalize_order_batch_cancel_idempotency_key(batch_key: &str, order_id: &str) -> String {
    let batch = normalize_order_idempotency_key(batch_key);
    let order = normalize_order_idempotency_key(order_id);
    if batch.is_empty() {
        return format!("batch-cancel::{order}");
    }
    format!("batch-cancel::{batch}::{order}")
}

pub fn is_duplicate_order_idempotency_key(existing_keys: &[String], candidate: &str) -> bool {
    let normalized_candidate = normalize_order_idempotency_key(candidate);
    existing_keys
        .iter()
        .any(|key| normalize_order_idempotency_key(key) == normalized_candidate)
}

pub fn validate_order_transition(
    from_state: Option<OrderLifecycleState>,
    to_state: OrderLifecycleState,
) -> Result<(), OrderLifecycleContractError> {
    if let Some(from_state) = from_state {
        if from_state.is_terminal() {
            return Err(OrderLifecycleContractError::already_terminal(format!(
                "terminal state `{}` cannot transition to `{}`",
                from_state.as_str(),
                to_state.as_str()
            )));
        }
        let is_allowed = match from_state {
            OrderLifecycleState::Pending => matches!(
                to_state,
                OrderLifecycleState::Pending
                    | OrderLifecycleState::Live
                    | OrderLifecycleState::Canceled
                    | OrderLifecycleState::Expired
            ),
            OrderLifecycleState::Live => matches!(
                to_state,
                OrderLifecycleState::Live
                    | OrderLifecycleState::PartiallyFilled
                    | OrderLifecycleState::Filled
                    | OrderLifecycleState::Canceled
                    | OrderLifecycleState::Expired
            ),
            OrderLifecycleState::PartiallyFilled => matches!(
                to_state,
                OrderLifecycleState::PartiallyFilled
                    | OrderLifecycleState::Filled
                    | OrderLifecycleState::Canceled
                    | OrderLifecycleState::Expired
            ),
            OrderLifecycleState::Filled
            | OrderLifecycleState::Canceled
            | OrderLifecycleState::Expired => false,
        };
        if !is_allowed {
            return Err(OrderLifecycleContractError::invalid_payload(format!(
                "invalid lifecycle transition `{}` -> `{}`",
                from_state.as_str(),
                to_state.as_str()
            )));
        }
    }
    Ok(())
}

pub fn validate_order_record(record: &OrderRecord) -> Result<(), OrderLifecycleContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "order_id", &record.order_id);
    validate_non_empty_field(&mut field_errors, "market_id", &record.market_id);
    validate_non_empty_field(
        &mut field_errors,
        "submission_idempotency_key",
        &record.submission_idempotency_key,
    );
    validate_non_empty_field(
        &mut field_errors,
        "last_reason_code",
        &record.last_reason_code,
    );
    validate_non_empty_field(&mut field_errors, "correlation_id", &record.correlation_id);
    validate_timestamp_field(&mut field_errors, "created_at_utc", &record.created_at_utc);
    validate_timestamp_field(&mut field_errors, "updated_at_utc", &record.updated_at_utc);

    if normalize_order_idempotency_key(&record.submission_idempotency_key)
        != record.submission_idempotency_key
    {
        field_errors.push(OrderLifecycleValidationIssue {
            field: "submission_idempotency_key",
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: "submission_idempotency_key must be normalized (trimmed lowercase)"
                .to_string(),
        });
    }
    if let Some(cancel_key) = record.cancel_idempotency_key.as_ref() {
        validate_non_empty_field(&mut field_errors, "cancel_idempotency_key", cancel_key);
        if normalize_order_idempotency_key(cancel_key) != *cancel_key {
            field_errors.push(OrderLifecycleValidationIssue {
                field: "cancel_idempotency_key",
                code: OrderLifecycleReasonCode::InvalidPayload.code(),
                message: "cancel_idempotency_key must be normalized (trimmed lowercase)"
                    .to_string(),
            });
        }
    }
    if OrderLifecycleReasonCode::parse(&record.last_reason_code).is_err() {
        field_errors.push(OrderLifecycleValidationIssue {
            field: "last_reason_code",
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: "last_reason_code must be a known order lifecycle reason".to_string(),
        });
    }
    if record.last_transition_sequence <= 0 {
        field_errors.push(OrderLifecycleValidationIssue {
            field: "last_transition_sequence",
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: "last_transition_sequence must be greater than 0".to_string(),
        });
    }
    if let (Ok(created_at), Ok(updated_at)) = (
        parse_utc_timestamp(&record.created_at_utc),
        parse_utc_timestamp(&record.updated_at_utc),
    ) && updated_at < created_at
    {
        field_errors.push(OrderLifecycleValidationIssue {
            field: "updated_at_utc",
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: "updated_at_utc cannot be earlier than created_at_utc".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(OrderLifecycleContractError::invalid_payload_with_issues(
            "order record is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_order_state_transition(
    transition: &OrderStateTransition,
) -> Result<(), OrderLifecycleContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(
        &mut field_errors,
        "transition_id",
        &transition.transition_id,
    );
    validate_non_empty_field(&mut field_errors, "order_id", &transition.order_id);
    validate_non_empty_field(&mut field_errors, "market_id", &transition.market_id);
    validate_non_empty_field(&mut field_errors, "reason_code", &transition.reason_code);
    validate_non_empty_field(
        &mut field_errors,
        "idempotency_key",
        &transition.idempotency_key,
    );
    validate_non_empty_field(
        &mut field_errors,
        "correlation_id",
        &transition.correlation_id,
    );
    validate_timestamp_field(
        &mut field_errors,
        "transitioned_at_utc",
        &transition.transitioned_at_utc,
    );
    if transition.transition_sequence <= 0 {
        field_errors.push(OrderLifecycleValidationIssue {
            field: "transition_sequence",
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: "transition_sequence must be greater than 0".to_string(),
        });
    }
    if normalize_order_idempotency_key(&transition.idempotency_key) != transition.idempotency_key {
        field_errors.push(OrderLifecycleValidationIssue {
            field: "idempotency_key",
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: "idempotency_key must be normalized (trimmed lowercase)".to_string(),
        });
    }
    if transition.from_state.is_none() && transition.transition_sequence != 1 {
        field_errors.push(OrderLifecycleValidationIssue {
            field: "transition_sequence",
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: "first transition must use sequence 1".to_string(),
        });
    }
    if OrderLifecycleReasonCode::parse(&transition.reason_code).is_err() {
        field_errors.push(OrderLifecycleValidationIssue {
            field: "reason_code",
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known order lifecycle reason".to_string(),
        });
    }
    if let Err(error) = validate_order_transition(transition.from_state, transition.to_state) {
        field_errors.push(OrderLifecycleValidationIssue {
            field: "to_state",
            code: error.code,
            message: error.message,
        });
    }

    if !field_errors.is_empty() {
        return Err(OrderLifecycleContractError::invalid_payload_with_issues(
            "order state transition is invalid",
            field_errors,
        ));
    }
    Ok(())
}

fn validate_non_empty_field(
    field_errors: &mut Vec<OrderLifecycleValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(OrderLifecycleValidationIssue {
            field,
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_timestamp_field(
    field_errors: &mut Vec<OrderLifecycleValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(OrderLifecycleValidationIssue {
            field,
            code: OrderLifecycleReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, ()> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ())?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(());
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_order_record() -> OrderRecord {
        OrderRecord {
            order_id: "order-1".to_string(),
            market_id: "market-1".to_string(),
            mode: OrderMode::Limit,
            state: OrderLifecycleState::Live,
            submission_idempotency_key: "submit::order-1".to_string(),
            cancel_idempotency_key: None,
            last_transition_sequence: 2,
            last_reason_code: OrderLifecycleReasonCode::VenueUpdateAccepted
                .code()
                .to_string(),
            correlation_id: "corr-order-1".to_string(),
            created_at_utc: "2026-04-06T00:00:00Z".to_string(),
            updated_at_utc: "2026-04-06T00:00:01Z".to_string(),
        }
    }

    #[test]
    fn lifecycle_happy_path_progresses_to_terminal_resolution() {
        assert!(validate_order_transition(None, OrderLifecycleState::Pending).is_ok());
        assert!(
            validate_order_transition(
                Some(OrderLifecycleState::Pending),
                OrderLifecycleState::Live
            )
            .is_ok()
        );
        assert!(
            validate_order_transition(
                Some(OrderLifecycleState::Live),
                OrderLifecycleState::PartiallyFilled
            )
            .is_ok()
        );
        assert!(
            validate_order_transition(
                Some(OrderLifecycleState::PartiallyFilled),
                OrderLifecycleState::Filled
            )
            .is_ok()
        );
    }

    #[test]
    fn invalid_transition_is_rejected_with_machine_readable_reason() {
        let error = validate_order_transition(
            Some(OrderLifecycleState::Live),
            OrderLifecycleState::Pending,
        )
        .expect_err("live -> pending must be rejected");
        assert_eq!(error.code, OrderLifecycleReasonCode::InvalidPayload.code());
        assert!(error.message.contains("invalid lifecycle transition"));
    }

    #[test]
    fn terminal_state_is_immutable() {
        let error =
            validate_order_transition(Some(OrderLifecycleState::Filled), OrderLifecycleState::Live)
                .expect_err("terminal state should reject non-terminal transitions");
        assert_eq!(error.code, OrderLifecycleReasonCode::AlreadyTerminal.code());
    }

    #[test]
    fn idempotency_normalization_helpers_are_deterministic() {
        assert_eq!(
            normalize_order_submission_idempotency_key("order-1", " Submit::Order-1 "),
            "submit::order-1"
        );
        assert_eq!(
            normalize_order_cancel_idempotency_key("order-1", " Cancel::Order-1 "),
            "cancel::order-1"
        );
        assert_eq!(
            normalize_order_batch_cancel_idempotency_key(" Batch-1 ", " Order-1 "),
            "batch-cancel::batch-1::order-1"
        );
    }

    #[test]
    fn duplicate_idempotency_detection_is_case_and_whitespace_insensitive() {
        let keys = vec!["submit::order-1".to_string(), "cancel::order-1".to_string()];
        assert!(is_duplicate_order_idempotency_key(
            &keys,
            "  CANCEL::ORDER-1  "
        ));
        assert!(!is_duplicate_order_idempotency_key(
            &keys,
            "submit::order-2"
        ));
    }

    #[test]
    fn order_record_validation_rejects_non_utc_timestamp() {
        let mut record = sample_order_record();
        record.updated_at_utc = "2026-04-06T00:00:01+01:00".to_string();
        let error =
            validate_order_record(&record).expect_err("non-UTC order record timestamp should fail");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "updated_at_utc")
        );
    }

    #[test]
    fn order_state_transition_validation_enforces_terminal_boundary() {
        let transition = OrderStateTransition {
            transition_id: "transition-3".to_string(),
            order_id: "order-1".to_string(),
            market_id: "market-1".to_string(),
            mode: OrderMode::Limit,
            from_state: Some(OrderLifecycleState::Filled),
            to_state: OrderLifecycleState::Live,
            reason_code: OrderLifecycleReasonCode::TransitionRejected
                .code()
                .to_string(),
            idempotency_key: "venue::order-1::3".to_string(),
            correlation_id: "corr-order-1".to_string(),
            transition_sequence: 3,
            transitioned_at_utc: "2026-04-06T00:00:03Z".to_string(),
        };

        let error = validate_order_state_transition(&transition)
            .expect_err("terminal -> non-terminal transition should fail");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.code == OrderLifecycleReasonCode::AlreadyTerminal.code())
        );
    }
}
