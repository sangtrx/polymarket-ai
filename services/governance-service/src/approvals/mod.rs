use domain::governance::{
    ApprovalDecisionEvidence, ApprovalDecisionOutcome, ApprovalReasonCode, ApprovalRequest,
    ApprovalState, ApprovalVoteDecision, ApprovalVoteRecord, CriticalActionId, actors_are_distinct,
    approval_window_expired, canonical_actor_id, generate_approval_reference,
};
use persistence::postgres::approvals::{
    ApprovalPersistenceError, count_actor_requests_since as pg_count_actor_requests_since,
    create_approval_request as pg_create_approval_request,
    load_approval_request as pg_load_approval_request,
    load_approval_votes as pg_load_approval_votes, record_approval_vote as pg_record_approval_vote,
    update_approval_request as pg_update_approval_request,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

const MAX_REQUESTS_PER_HOUR: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalServiceError {
    pub code: &'static str,
    pub message: String,
}

impl ApprovalServiceError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "approval_invalid_payload",
            message: message.into(),
        }
    }

    pub fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: "approval_persistence_unavailable",
            message: message.into(),
        }
    }

    pub fn duplicate_request(request_id: &str) -> Self {
        Self {
            code: "approval_duplicate_request",
            message: format!("approval request `{request_id}` already exists"),
        }
    }
}

impl Display for ApprovalServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ApprovalServiceError {}

fn map_persistence_error(error: ApprovalPersistenceError) -> ApprovalServiceError {
    match error.code {
        "approval_query_failed" | "approval_row_decode_failed" => {
            ApprovalServiceError::persistence_unavailable(error.message)
        }
        _ => ApprovalServiceError {
            code: error.code,
            message: error.message,
        },
    }
}

pub trait ApprovalRepositoryPort: Send + Sync {
    fn create_request(&self, request: ApprovalRequest) -> Result<(), ApprovalServiceError>;
    fn fetch_request(
        &self,
        request_id: &str,
    ) -> Result<Option<ApprovalRequest>, ApprovalServiceError>;
    fn update_request(
        &self,
        request_id: &str,
        status: ApprovalState,
        reason_code: ApprovalReasonCode,
        approval_reference: Option<String>,
    ) -> Result<(), ApprovalServiceError>;
    fn record_vote(&self, vote: ApprovalVoteRecord) -> Result<(), ApprovalServiceError>;
    fn fetch_votes(
        &self,
        request_id: &str,
    ) -> Result<Vec<ApprovalVoteRecord>, ApprovalServiceError>;
    fn count_actor_requests_since(
        &self,
        actor_id: &str,
        since_utc: &str,
    ) -> Result<u32, ApprovalServiceError>;
}

pub trait ApprovalOrchestrator: Send + Sync {
    fn submit_request(
        &self,
        input: SubmitApprovalRequestInput,
    ) -> Result<ApprovalDecisionEvidence, ApprovalServiceError>;
    fn record_vote(
        &self,
        input: RecordApprovalVoteInput,
    ) -> Result<ApprovalDecisionEvidence, ApprovalServiceError>;
    fn evaluate_execution(
        &self,
        input: EvaluateApprovalExecutionInput,
    ) -> Result<ApprovalDecisionEvidence, ApprovalServiceError>;
}

#[derive(Debug, Clone)]
pub struct SubmitApprovalRequestInput {
    pub request_id: String,
    pub action_id: String,
    pub proposer_actor_id: String,
    pub proposer_role: String,
    pub correlation_id: String,
    pub now_utc: String,
    pub expires_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct RecordApprovalVoteInput {
    pub request_id: String,
    pub action_id: String,
    pub actor_id: String,
    pub actor_role: String,
    pub vote: ApprovalVoteDecision,
    pub correlation_id: String,
    pub now_utc: String,
}

#[derive(Debug, Clone)]
pub struct EvaluateApprovalExecutionInput {
    pub request_id: String,
    pub action_id: String,
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub now_utc: String,
}

#[derive(Clone)]
pub struct GovernanceApprovalService {
    repository: Arc<dyn ApprovalRepositoryPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl GovernanceApprovalService {
    pub fn new(repository: Arc<dyn ApprovalRepositoryPort>) -> Self {
        Self {
            repository,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(Arc::new(PostgresApprovalRepository::new(pool)))
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryApprovalRepository::default()))
    }

    fn lock_operations(&self) -> Result<std::sync::MutexGuard<'_, ()>, ApprovalServiceError> {
        self.operation_lock.lock().map_err(|_| {
            ApprovalServiceError::persistence_unavailable(
                "approval operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for GovernanceApprovalService {
    fn default() -> Self {
        Self::in_memory()
    }
}

#[derive(Debug, Clone)]
pub struct PostgresApprovalRepository {
    pool: PgPool,
}

impl PostgresApprovalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, ApprovalServiceError>
    where
        F: Future<Output = Result<T, ApprovalPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    ApprovalServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl ApprovalRepositoryPort for PostgresApprovalRepository {
    fn create_request(&self, request: ApprovalRequest) -> Result<(), ApprovalServiceError> {
        self.run_with_runtime(pg_create_approval_request(&self.pool, &request))
    }

    fn fetch_request(
        &self,
        request_id: &str,
    ) -> Result<Option<ApprovalRequest>, ApprovalServiceError> {
        self.run_with_runtime(pg_load_approval_request(&self.pool, request_id))
    }

    fn update_request(
        &self,
        request_id: &str,
        status: ApprovalState,
        reason_code: ApprovalReasonCode,
        approval_reference: Option<String>,
    ) -> Result<(), ApprovalServiceError> {
        self.run_with_runtime(pg_update_approval_request(
            &self.pool,
            request_id,
            status,
            reason_code.code(),
            approval_reference.as_deref(),
        ))
    }

    fn record_vote(&self, vote: ApprovalVoteRecord) -> Result<(), ApprovalServiceError> {
        self.run_with_runtime(pg_record_approval_vote(&self.pool, &vote))
    }

    fn fetch_votes(
        &self,
        request_id: &str,
    ) -> Result<Vec<ApprovalVoteRecord>, ApprovalServiceError> {
        self.run_with_runtime(pg_load_approval_votes(&self.pool, request_id))
    }

    fn count_actor_requests_since(
        &self,
        actor_id: &str,
        since_utc: &str,
    ) -> Result<u32, ApprovalServiceError> {
        self.run_with_runtime(pg_count_actor_requests_since(
            &self.pool, actor_id, since_utc,
        ))
    }
}

impl ApprovalOrchestrator for GovernanceApprovalService {
    fn submit_request(
        &self,
        input: SubmitApprovalRequestInput,
    ) -> Result<ApprovalDecisionEvidence, ApprovalServiceError> {
        let action = CriticalActionId::parse(input.action_id.as_str())
            .map_err(|error| ApprovalServiceError::invalid_payload(error.message))?;
        validate_actor_role(&input.proposer_role)?;
        validate_non_empty("request_id", &input.request_id)?;
        validate_non_empty("proposer_actor_id", &input.proposer_actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        parse_utc("now_utc", &input.now_utc)?;
        parse_utc("expires_at_utc", &input.expires_at_utc)?;
        let _operation_guard = self.lock_operations()?;
        let canonical_proposer_actor_id = canonical_actor_id(&input.proposer_actor_id);

        if approval_window_expired(&input.now_utc, &input.expires_at_utc)
            .map_err(|error| ApprovalServiceError::invalid_payload(error.message))?
        {
            let decision = deny_decision(
                Some(input.request_id),
                input.proposer_actor_id,
                action.as_str(),
                ApprovalState::Expired,
                ApprovalReasonCode::ExpiredWindow,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        if self.repository.fetch_request(&input.request_id)?.is_some() {
            return Err(ApprovalServiceError::duplicate_request(&input.request_id));
        }

        let since_utc = (parse_utc("now_utc", &input.now_utc)? - Duration::hours(1))
            .format(&Rfc3339)
            .expect("RFC3339 formatting should always succeed");
        let recent_count = self
            .repository
            .count_actor_requests_since(&canonical_proposer_actor_id, &since_utc)?;
        if recent_count >= MAX_REQUESTS_PER_HOUR {
            let decision = deny_decision(
                Some(input.request_id),
                input.proposer_actor_id,
                action.as_str(),
                ApprovalState::Pending,
                ApprovalReasonCode::RateLimitedActor,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        let request = ApprovalRequest {
            request_id: input.request_id.clone(),
            action_id: action.as_str().to_string(),
            proposer_actor_id: canonical_proposer_actor_id,
            status: ApprovalState::Pending,
            reason_code: ApprovalReasonCode::ApprovalPending.code().to_string(),
            correlation_id: input.correlation_id.clone(),
            approval_reference: None,
            created_at_utc: input.now_utc.clone(),
            expires_at_utc: input.expires_at_utc,
        };
        if let Err(error) = self.repository.create_request(request) {
            if error.code == "approval_duplicate_request" {
                return Err(ApprovalServiceError::duplicate_request(&input.request_id));
            }
            return Err(error);
        }

        let decision = ApprovalDecisionEvidence {
            request_id: Some(input.request_id),
            actor_id: input.proposer_actor_id,
            action_id: action.as_str().to_string(),
            outcome: ApprovalDecisionOutcome::Pending,
            state: ApprovalState::Pending,
            reason_code: ApprovalReasonCode::ApprovalPending.code().to_string(),
            correlation_id: input.correlation_id,
            approval_reference: None,
            timestamp_utc: input.now_utc,
        };
        emit_approval_telemetry(&decision);
        Ok(decision)
    }

    fn record_vote(
        &self,
        input: RecordApprovalVoteInput,
    ) -> Result<ApprovalDecisionEvidence, ApprovalServiceError> {
        let action = CriticalActionId::parse(input.action_id.as_str())
            .map_err(|error| ApprovalServiceError::invalid_payload(error.message))?;
        validate_actor_role(&input.actor_role)?;
        validate_non_empty("request_id", &input.request_id)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        parse_utc("now_utc", &input.now_utc)?;
        let _operation_guard = self.lock_operations()?;
        let canonical_actor = canonical_actor_id(&input.actor_id);

        let Some(request) = self.repository.fetch_request(&input.request_id)? else {
            let decision = deny_decision(
                Some(input.request_id),
                input.actor_id,
                action.as_str(),
                ApprovalState::Pending,
                ApprovalReasonCode::UnknownRequest,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        };

        if request.action_id != action.as_str() {
            let decision = deny_decision(
                Some(request.request_id),
                input.actor_id,
                action.as_str(),
                request.status,
                ApprovalReasonCode::InvalidStateTransition,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        if request
            .is_expired_at(&input.now_utc)
            .map_err(|error| ApprovalServiceError::invalid_payload(error.message))?
        {
            self.repository.update_request(
                &request.request_id,
                ApprovalState::Expired,
                ApprovalReasonCode::ExpiredWindow,
                None,
            )?;
            let decision = deny_decision(
                Some(request.request_id),
                input.actor_id,
                action.as_str(),
                ApprovalState::Expired,
                ApprovalReasonCode::ExpiredWindow,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        let existing_votes = self.repository.fetch_votes(&request.request_id)?;
        if existing_votes
            .iter()
            .any(|vote| canonical_actor_id(&vote.actor_id) == canonical_actor)
        {
            let decision = deny_decision(
                Some(request.request_id),
                input.actor_id,
                action.as_str(),
                ApprovalState::Pending,
                ApprovalReasonCode::DuplicateVote,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        if request.status != ApprovalState::Pending {
            let decision = deny_decision(
                Some(request.request_id),
                input.actor_id,
                action.as_str(),
                request.status,
                ApprovalReasonCode::InvalidStateTransition,
                input.correlation_id,
                input.now_utc,
                request.approval_reference,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        if !actors_are_distinct(&request.proposer_actor_id, &input.actor_id) {
            let decision = deny_decision(
                Some(request.request_id),
                input.actor_id,
                action.as_str(),
                ApprovalState::Pending,
                ApprovalReasonCode::SelfApprovalAttempt,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        if let Err(error) = self.repository.record_vote(ApprovalVoteRecord {
            request_id: request.request_id.clone(),
            actor_id: canonical_actor.clone(),
            decision: input.vote,
            correlation_id: input.correlation_id.clone(),
            voted_at_utc: input.now_utc.clone(),
        }) {
            if error.code == ApprovalReasonCode::DuplicateVote.code() {
                let decision = deny_decision(
                    Some(request.request_id),
                    input.actor_id,
                    action.as_str(),
                    ApprovalState::Pending,
                    ApprovalReasonCode::DuplicateVote,
                    input.correlation_id,
                    input.now_utc,
                    None,
                );
                emit_approval_telemetry(&decision);
                return Ok(decision);
            }
            return Err(error);
        }

        if input.vote == ApprovalVoteDecision::Reject {
            self.repository.update_request(
                &request.request_id,
                ApprovalState::Rejected,
                ApprovalReasonCode::ExplicitlyRejected,
                None,
            )?;
            let decision = deny_decision(
                Some(request.request_id),
                input.actor_id,
                action.as_str(),
                ApprovalState::Rejected,
                ApprovalReasonCode::ExplicitlyRejected,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        let approval_reference = generate_approval_reference(
            &request.request_id,
            action,
            &request.proposer_actor_id,
            &canonical_actor,
            &input.now_utc,
        )
        .map_err(|error| ApprovalServiceError::invalid_payload(error.message))?;
        self.repository.update_request(
            &request.request_id,
            ApprovalState::Approved,
            ApprovalReasonCode::ApprovalAllowed,
            Some(approval_reference.clone()),
        )?;

        let decision = allow_decision(
            Some(request.request_id),
            input.actor_id,
            action.as_str(),
            ApprovalState::Approved,
            ApprovalReasonCode::ApprovalAllowed,
            input.correlation_id,
            input.now_utc,
            Some(approval_reference),
        );
        emit_approval_telemetry(&decision);
        Ok(decision)
    }

    fn evaluate_execution(
        &self,
        input: EvaluateApprovalExecutionInput,
    ) -> Result<ApprovalDecisionEvidence, ApprovalServiceError> {
        let action = CriticalActionId::parse(input.action_id.as_str())
            .map_err(|error| ApprovalServiceError::invalid_payload(error.message))?;
        validate_actor_role(&input.actor_role)?;
        validate_non_empty("request_id", &input.request_id)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        parse_utc("now_utc", &input.now_utc)?;
        let _operation_guard = self.lock_operations()?;

        let Some(request) = self.repository.fetch_request(&input.request_id)? else {
            let decision = deny_decision(
                Some(input.request_id),
                input.actor_id,
                action.as_str(),
                ApprovalState::Pending,
                ApprovalReasonCode::MissingApprovalRequest,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        };

        if request.action_id != action.as_str() {
            let decision = deny_decision(
                Some(request.request_id),
                input.actor_id,
                action.as_str(),
                request.status,
                ApprovalReasonCode::MissingApprovalRequest,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        if request
            .is_expired_at(&input.now_utc)
            .map_err(|error| ApprovalServiceError::invalid_payload(error.message))?
        {
            self.repository.update_request(
                &request.request_id,
                ApprovalState::Expired,
                ApprovalReasonCode::ExpiredWindow,
                None,
            )?;
            let decision = deny_decision(
                Some(request.request_id),
                input.actor_id,
                action.as_str(),
                ApprovalState::Expired,
                ApprovalReasonCode::ExpiredWindow,
                input.correlation_id,
                input.now_utc,
                None,
            );
            emit_approval_telemetry(&decision);
            return Ok(decision);
        }

        match request.status {
            ApprovalState::Approved => {
                if request.approval_reference.is_none() {
                    let decision = deny_decision(
                        Some(request.request_id),
                        input.actor_id,
                        action.as_str(),
                        ApprovalState::Approved,
                        ApprovalReasonCode::InvalidStateTransition,
                        input.correlation_id,
                        input.now_utc,
                        None,
                    );
                    emit_approval_telemetry(&decision);
                    return Ok(decision);
                }
                let decision = allow_decision(
                    Some(request.request_id),
                    input.actor_id,
                    action.as_str(),
                    ApprovalState::Approved,
                    ApprovalReasonCode::ApprovalAllowed,
                    input.correlation_id,
                    input.now_utc,
                    request.approval_reference,
                );
                emit_approval_telemetry(&decision);
                Ok(decision)
            }
            ApprovalState::Pending => {
                let votes = self.repository.fetch_votes(&request.request_id)?;

                let has_distinct_reject_vote = votes.iter().any(|vote| {
                    vote.decision == ApprovalVoteDecision::Reject
                        && actors_are_distinct(&request.proposer_actor_id, &vote.actor_id)
                });
                if has_distinct_reject_vote {
                    self.repository.update_request(
                        &request.request_id,
                        ApprovalState::Rejected,
                        ApprovalReasonCode::ExplicitlyRejected,
                        None,
                    )?;
                    let decision = deny_decision(
                        Some(request.request_id),
                        input.actor_id,
                        action.as_str(),
                        ApprovalState::Rejected,
                        ApprovalReasonCode::ExplicitlyRejected,
                        input.correlation_id,
                        input.now_utc,
                        None,
                    );
                    emit_approval_telemetry(&decision);
                    return Ok(decision);
                }

                let distinct_approve_vote = votes.iter().find(|vote| {
                    vote.decision == ApprovalVoteDecision::Approve
                        && actors_are_distinct(&request.proposer_actor_id, &vote.actor_id)
                });
                let Some(approve_vote) = distinct_approve_vote else {
                    let decision = deny_decision(
                        Some(request.request_id),
                        input.actor_id,
                        action.as_str(),
                        ApprovalState::Pending,
                        ApprovalReasonCode::MissingSecondApprover,
                        input.correlation_id,
                        input.now_utc,
                        None,
                    );
                    emit_approval_telemetry(&decision);
                    return Ok(decision);
                };

                let approval_reference = generate_approval_reference(
                    &request.request_id,
                    action,
                    &request.proposer_actor_id,
                    &canonical_actor_id(&approve_vote.actor_id),
                    &approve_vote.voted_at_utc,
                )
                .map_err(|error| ApprovalServiceError::invalid_payload(error.message))?;
                self.repository.update_request(
                    &request.request_id,
                    ApprovalState::Approved,
                    ApprovalReasonCode::ApprovalAllowed,
                    Some(approval_reference.clone()),
                )?;

                let decision = allow_decision(
                    Some(request.request_id),
                    input.actor_id,
                    action.as_str(),
                    ApprovalState::Approved,
                    ApprovalReasonCode::ApprovalAllowed,
                    input.correlation_id,
                    input.now_utc,
                    Some(approval_reference),
                );
                emit_approval_telemetry(&decision);
                Ok(decision)
            }
            ApprovalState::Rejected => {
                let decision = deny_decision(
                    Some(request.request_id),
                    input.actor_id,
                    action.as_str(),
                    ApprovalState::Rejected,
                    ApprovalReasonCode::ExplicitlyRejected,
                    input.correlation_id,
                    input.now_utc,
                    None,
                );
                emit_approval_telemetry(&decision);
                Ok(decision)
            }
            ApprovalState::Expired => {
                let decision = deny_decision(
                    Some(request.request_id),
                    input.actor_id,
                    action.as_str(),
                    ApprovalState::Expired,
                    ApprovalReasonCode::ExpiredWindow,
                    input.correlation_id,
                    input.now_utc,
                    None,
                );
                emit_approval_telemetry(&decision);
                Ok(decision)
            }
        }
    }
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), ApprovalServiceError> {
    if value.trim().is_empty() {
        return Err(ApprovalServiceError::invalid_payload(format!(
            "{field} cannot be blank"
        )));
    }
    Ok(())
}

fn validate_actor_role(role: &str) -> Result<(), ApprovalServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(ApprovalServiceError {
            code: ApprovalReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for critical approval workflow".to_string(),
        }),
    }
}

fn parse_utc(field: &'static str, value: &str) -> Result<OffsetDateTime, ApprovalServiceError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        ApprovalServiceError::invalid_payload(format!(
            "`{field}` must be RFC3339 UTC timestamp; got `{value}`"
        ))
    })?;
    if parsed.offset() != time::UtcOffset::UTC {
        return Err(ApprovalServiceError::invalid_payload(format!(
            "`{field}` must use UTC `Z` offset"
        )));
    }
    Ok(parsed)
}

#[allow(clippy::too_many_arguments)]
fn allow_decision(
    request_id: Option<String>,
    actor_id: String,
    action_id: &str,
    state: ApprovalState,
    reason_code: ApprovalReasonCode,
    correlation_id: String,
    timestamp_utc: String,
    approval_reference: Option<String>,
) -> ApprovalDecisionEvidence {
    ApprovalDecisionEvidence {
        request_id,
        actor_id,
        action_id: action_id.to_string(),
        outcome: ApprovalDecisionOutcome::Allow,
        state,
        reason_code: reason_code.code().to_string(),
        correlation_id,
        approval_reference,
        timestamp_utc,
    }
}

#[allow(clippy::too_many_arguments)]
fn deny_decision(
    request_id: Option<String>,
    actor_id: String,
    action_id: &str,
    state: ApprovalState,
    reason_code: ApprovalReasonCode,
    correlation_id: String,
    timestamp_utc: String,
    approval_reference: Option<String>,
) -> ApprovalDecisionEvidence {
    ApprovalDecisionEvidence {
        request_id,
        actor_id,
        action_id: action_id.to_string(),
        outcome: ApprovalDecisionOutcome::Deny,
        state,
        reason_code: reason_code.code().to_string(),
        correlation_id,
        approval_reference,
        timestamp_utc,
    }
}

fn emit_approval_telemetry(decision: &ApprovalDecisionEvidence) {
    let telemetry_event = ApprovalTelemetryEvent {
        event_name: "critical_action_approval_decision_v1",
        request_id: decision.request_id.as_deref(),
        actor_id: &decision.actor_id,
        action: &decision.action_id,
        outcome: decision.outcome.as_str(),
        state: decision.state.as_str(),
        reason_code: &decision.reason_code,
        correlation_id: &decision.correlation_id,
        timestamp_utc: &decision.timestamp_utc,
        approval_reference: decision.approval_reference.as_deref(),
        security_signal: match decision.outcome {
            ApprovalDecisionOutcome::Deny => Some(ApprovalSecuritySignal {
                name: "unauthorized_privileged_approval_attempt_v1",
                alert_compatible: true,
                alert_target_seconds: 30,
                severity: "high",
            }),
            _ => None,
        },
    };

    println!(
        "{}",
        serde_json::to_string(&telemetry_event)
            .expect("critical-approval telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct ApprovalTelemetryEvent<'a> {
    event_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<&'a str>,
    actor_id: &'a str,
    action: &'a str,
    outcome: &'a str,
    state: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_reference: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<ApprovalSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct ApprovalSecuritySignal<'a> {
    name: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
    severity: &'a str,
}

#[derive(Debug, Default)]
pub struct InMemoryApprovalRepository {
    requests: Mutex<BTreeMap<String, ApprovalRequest>>,
    votes: Mutex<Vec<ApprovalVoteRecord>>,
}

impl ApprovalRepositoryPort for InMemoryApprovalRepository {
    fn create_request(&self, request: ApprovalRequest) -> Result<(), ApprovalServiceError> {
        let mut requests = self
            .requests
            .lock()
            .expect("in-memory approval requests lock should not be poisoned");
        if requests.contains_key(&request.request_id) {
            return Err(ApprovalServiceError::duplicate_request(&request.request_id));
        }
        requests.insert(request.request_id.clone(), request);
        Ok(())
    }

    fn fetch_request(
        &self,
        request_id: &str,
    ) -> Result<Option<ApprovalRequest>, ApprovalServiceError> {
        let requests = self
            .requests
            .lock()
            .expect("in-memory approval requests lock should not be poisoned");
        Ok(requests.get(request_id).cloned())
    }

    fn update_request(
        &self,
        request_id: &str,
        status: ApprovalState,
        reason_code: ApprovalReasonCode,
        approval_reference: Option<String>,
    ) -> Result<(), ApprovalServiceError> {
        let mut requests = self
            .requests
            .lock()
            .expect("in-memory approval requests lock should not be poisoned");
        let request = requests.get_mut(request_id).ok_or_else(|| {
            ApprovalServiceError::invalid_payload(format!(
                "approval request `{request_id}` does not exist"
            ))
        })?;
        request.status = status;
        request.reason_code = reason_code.code().to_string();
        request.approval_reference = approval_reference;
        Ok(())
    }

    fn record_vote(&self, vote: ApprovalVoteRecord) -> Result<(), ApprovalServiceError> {
        let mut votes = self
            .votes
            .lock()
            .expect("in-memory approval votes lock should not be poisoned");
        votes.push(vote);
        Ok(())
    }

    fn fetch_votes(
        &self,
        request_id: &str,
    ) -> Result<Vec<ApprovalVoteRecord>, ApprovalServiceError> {
        let votes = self
            .votes
            .lock()
            .expect("in-memory approval votes lock should not be poisoned");
        Ok(votes
            .iter()
            .filter(|vote| vote.request_id == request_id)
            .cloned()
            .collect())
    }

    fn count_actor_requests_since(
        &self,
        actor_id: &str,
        since_utc: &str,
    ) -> Result<u32, ApprovalServiceError> {
        let since = parse_utc("since_utc", since_utc)?;
        let requests = self
            .requests
            .lock()
            .expect("in-memory approval requests lock should not be poisoned");
        let count = requests
            .values()
            .filter(|request| {
                canonical_actor_id(&request.proposer_actor_id) == canonical_actor_id(actor_id)
            })
            .filter_map(|request| {
                OffsetDateTime::parse(&request.created_at_utc, &Rfc3339)
                    .ok()
                    .map(|timestamp| timestamp >= since)
            })
            .filter(|is_after_window| *is_after_window)
            .count();
        Ok(count as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submit_input(
        request_id: &str,
        actor_id: &str,
        correlation_id: &str,
        now_utc: &str,
        expires_at_utc: &str,
    ) -> SubmitApprovalRequestInput {
        SubmitApprovalRequestInput {
            request_id: request_id.to_string(),
            action_id: "risk_limit_increase".to_string(),
            proposer_actor_id: actor_id.to_string(),
            proposer_role: "operational_control".to_string(),
            correlation_id: correlation_id.to_string(),
            now_utc: now_utc.to_string(),
            expires_at_utc: expires_at_utc.to_string(),
        }
    }

    #[test]
    fn submit_request_rate_limit_boundary_allows_five_and_denies_sixth() {
        let service = GovernanceApprovalService::default();
        let now = "2026-04-05T00:00:00Z";
        let expiry = "2026-04-05T01:00:00Z";

        for index in 0..5 {
            let decision = service
                .submit_request(submit_input(
                    &format!("req-{index}"),
                    "ops-1",
                    &format!("corr-{index}"),
                    now,
                    expiry,
                ))
                .expect("submit should succeed");
            assert_eq!(decision.outcome, ApprovalDecisionOutcome::Pending);
        }

        let sixth = service
            .submit_request(submit_input("req-5", "ops-1", "corr-5", now, expiry))
            .expect("sixth request should return machine-readable denial");
        assert_eq!(sixth.outcome, ApprovalDecisionOutcome::Deny);
        assert_eq!(
            sixth.reason_code,
            ApprovalReasonCode::RateLimitedActor.code()
        );
    }

    #[test]
    fn vote_denies_self_approval_attempts() {
        let service = GovernanceApprovalService::default();
        service
            .submit_request(submit_input(
                "req-self",
                "ops-1",
                "corr-self-submit",
                "2026-04-05T00:00:00Z",
                "2026-04-05T01:00:00Z",
            ))
            .expect("submit should succeed");

        let decision = service
            .record_vote(RecordApprovalVoteInput {
                request_id: "req-self".to_string(),
                action_id: "risk_limit_increase".to_string(),
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                vote: ApprovalVoteDecision::Approve,
                correlation_id: "corr-self-vote".to_string(),
                now_utc: "2026-04-05T00:10:00Z".to_string(),
            })
            .expect("vote should return machine-readable denial");
        assert_eq!(decision.outcome, ApprovalDecisionOutcome::Deny);
        assert_eq!(
            decision.reason_code,
            ApprovalReasonCode::SelfApprovalAttempt.code()
        );
    }

    #[test]
    fn vote_denies_duplicate_actor_vote() {
        let service = GovernanceApprovalService::default();
        service
            .submit_request(submit_input(
                "req-dup",
                "ops-1",
                "corr-dup-submit",
                "2026-04-05T00:00:00Z",
                "2026-04-05T01:00:00Z",
            ))
            .expect("submit should succeed");

        let first_vote = service
            .record_vote(RecordApprovalVoteInput {
                request_id: "req-dup".to_string(),
                action_id: "risk_limit_increase".to_string(),
                actor_id: "admin-1".to_string(),
                actor_role: "administrative_actions".to_string(),
                vote: ApprovalVoteDecision::Reject,
                correlation_id: "corr-dup-vote-1".to_string(),
                now_utc: "2026-04-05T00:05:00Z".to_string(),
            })
            .expect("first vote should succeed");
        assert_eq!(first_vote.outcome, ApprovalDecisionOutcome::Deny);
        assert_eq!(
            first_vote.reason_code,
            ApprovalReasonCode::ExplicitlyRejected.code()
        );

        let second_vote = service
            .record_vote(RecordApprovalVoteInput {
                request_id: "req-dup".to_string(),
                action_id: "risk_limit_increase".to_string(),
                actor_id: "admin-1".to_string(),
                actor_role: "administrative_actions".to_string(),
                vote: ApprovalVoteDecision::Approve,
                correlation_id: "corr-dup-vote-2".to_string(),
                now_utc: "2026-04-05T00:06:00Z".to_string(),
            })
            .expect("second vote should return deterministic denial");
        assert_eq!(second_vote.outcome, ApprovalDecisionOutcome::Deny);
        assert_eq!(
            second_vote.reason_code,
            ApprovalReasonCode::DuplicateVote.code()
        );
    }

    #[test]
    fn vote_denies_expired_window_at_boundary() {
        let service = GovernanceApprovalService::default();
        service
            .submit_request(submit_input(
                "req-expired",
                "ops-1",
                "corr-expired-submit",
                "2026-04-05T00:00:00Z",
                "2026-04-05T00:05:00Z",
            ))
            .expect("submit should succeed");

        let decision = service
            .record_vote(RecordApprovalVoteInput {
                request_id: "req-expired".to_string(),
                action_id: "risk_limit_increase".to_string(),
                actor_id: "admin-1".to_string(),
                actor_role: "administrative_actions".to_string(),
                vote: ApprovalVoteDecision::Approve,
                correlation_id: "corr-expired-vote".to_string(),
                now_utc: "2026-04-05T00:05:00Z".to_string(),
            })
            .expect("boundary vote should deny");
        assert_eq!(decision.outcome, ApprovalDecisionOutcome::Deny);
        assert_eq!(
            decision.reason_code,
            ApprovalReasonCode::ExpiredWindow.code()
        );
    }

    #[test]
    fn evaluate_execution_requires_second_distinct_approver() {
        let service = GovernanceApprovalService::default();
        service
            .submit_request(submit_input(
                "req-pending",
                "ops-1",
                "corr-pending-submit",
                "2026-04-05T00:00:00Z",
                "2026-04-05T01:00:00Z",
            ))
            .expect("submit should succeed");

        let decision = service
            .evaluate_execution(EvaluateApprovalExecutionInput {
                request_id: "req-pending".to_string(),
                action_id: "risk_limit_increase".to_string(),
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-pending-execute".to_string(),
                now_utc: "2026-04-05T00:10:00Z".to_string(),
            })
            .expect("evaluation should return machine-readable denial");
        assert_eq!(decision.outcome, ApprovalDecisionOutcome::Deny);
        assert_eq!(
            decision.reason_code,
            ApprovalReasonCode::MissingSecondApprover.code()
        );
    }

    #[test]
    fn evaluate_execution_recovers_pending_state_with_distinct_approve_vote() {
        let repository = Arc::new(InMemoryApprovalRepository::default());
        repository
            .create_request(ApprovalRequest {
                request_id: "req-repair".to_string(),
                action_id: "risk_limit_increase".to_string(),
                proposer_actor_id: "ops-1".to_string(),
                status: ApprovalState::Pending,
                reason_code: ApprovalReasonCode::ApprovalPending.code().to_string(),
                correlation_id: "corr-repair-submit".to_string(),
                approval_reference: None,
                created_at_utc: "2026-04-05T00:00:00Z".to_string(),
                expires_at_utc: "2026-04-05T01:00:00Z".to_string(),
            })
            .expect("seed request should succeed");
        repository
            .record_vote(ApprovalVoteRecord {
                request_id: "req-repair".to_string(),
                actor_id: "admin-1".to_string(),
                decision: ApprovalVoteDecision::Approve,
                correlation_id: "corr-repair-vote".to_string(),
                voted_at_utc: "2026-04-05T00:01:00Z".to_string(),
            })
            .expect("seed vote should succeed");

        let service = GovernanceApprovalService::new(repository.clone());
        let decision = service
            .evaluate_execution(EvaluateApprovalExecutionInput {
                request_id: "req-repair".to_string(),
                action_id: "risk_limit_increase".to_string(),
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-repair-execute".to_string(),
                now_utc: "2026-04-05T00:02:00Z".to_string(),
            })
            .expect("execution should recover and allow");
        assert_eq!(decision.outcome, ApprovalDecisionOutcome::Allow);
        assert_eq!(decision.state, ApprovalState::Approved);
        assert_eq!(
            decision.reason_code,
            ApprovalReasonCode::ApprovalAllowed.code()
        );
        assert!(decision.approval_reference.is_some());

        let persisted = repository
            .fetch_request("req-repair")
            .expect("fetch request should succeed")
            .expect("request should exist");
        assert_eq!(persisted.status, ApprovalState::Approved);
        assert_eq!(
            persisted.reason_code,
            ApprovalReasonCode::ApprovalAllowed.code()
        );
        assert!(persisted.approval_reference.is_some());
    }

    #[test]
    fn evaluate_execution_returns_approval_reference_when_approved() {
        let service = GovernanceApprovalService::default();
        service
            .submit_request(submit_input(
                "req-approved",
                "ops-1",
                "corr-approved-submit",
                "2026-04-05T00:00:00Z",
                "2026-04-05T01:00:00Z",
            ))
            .expect("submit should succeed");
        let vote = service
            .record_vote(RecordApprovalVoteInput {
                request_id: "req-approved".to_string(),
                action_id: "risk_limit_increase".to_string(),
                actor_id: "admin-1".to_string(),
                actor_role: "administrative_actions".to_string(),
                vote: ApprovalVoteDecision::Approve,
                correlation_id: "corr-approved-vote".to_string(),
                now_utc: "2026-04-05T00:02:00Z".to_string(),
            })
            .expect("approval vote should succeed");
        assert_eq!(vote.outcome, ApprovalDecisionOutcome::Allow);
        assert!(vote.approval_reference.is_some());

        let decision = service
            .evaluate_execution(EvaluateApprovalExecutionInput {
                request_id: "req-approved".to_string(),
                action_id: "risk_limit_increase".to_string(),
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-approved-execute".to_string(),
                now_utc: "2026-04-05T00:03:00Z".to_string(),
            })
            .expect("execution should be allowed");
        assert_eq!(decision.outcome, ApprovalDecisionOutcome::Allow);
        assert_eq!(decision.state, ApprovalState::Approved);
        assert!(decision.approval_reference.is_some());
    }

    #[test]
    fn unauthorized_role_is_rejected_with_machine_code() {
        let service = GovernanceApprovalService::default();
        let error = service
            .submit_request(SubmitApprovalRequestInput {
                request_id: "req-unauthorized".to_string(),
                action_id: "risk_limit_increase".to_string(),
                proposer_actor_id: "reader-1".to_string(),
                proposer_role: "read_only_analytics".to_string(),
                correlation_id: "corr-unauthorized-submit".to_string(),
                now_utc: "2026-04-05T00:00:00Z".to_string(),
                expires_at_utc: "2026-04-05T01:00:00Z".to_string(),
            })
            .expect_err("read-only role should be rejected");
        assert_eq!(error.code, ApprovalReasonCode::UnauthorizedRole.code());
    }
}
