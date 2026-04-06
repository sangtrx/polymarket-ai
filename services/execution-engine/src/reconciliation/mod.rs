#![cfg_attr(not(test), allow(dead_code))]

use domain::order::OrderLifecycleState;
use domain::reconciliation::{
    ExposureSnapshot, ReconciliationContractError, ReconciliationDiffRecord,
    ReconciliationOrderRecord, ReconciliationReasonCode, ReconciliationRunSummary,
    authorize_reconciliation_read, build_exposure_snapshot, reconcile_window,
};
use persistence::postgres::reconciliation::{
    ReconciliationPersistenceError, load_latest_exposure_snapshot,
    load_latest_exposure_snapshot_for_run, load_reconciliation_diffs, load_reconciliation_run,
    persist_reconciliation_evidence,
};
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const LOAD_INTERNAL_TRUTH_WINDOW_SQL: &str = r#"
    SELECT
        o.order_id,
        o.market_id,
        o.lifecycle_state,
        COALESCE(
            to_char(latest_transition.transitioned_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
            to_char(o.updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        ) AS observed_at_utc,
        o.correlation_id
    FROM orders o
    LEFT JOIN LATERAL (
        SELECT transitioned_at_utc
        FROM order_state_transitions
        WHERE order_id = o.order_id
        ORDER BY transition_sequence DESC
        LIMIT 1
    ) latest_transition ON true
    WHERE COALESCE(latest_transition.transitioned_at_utc, o.updated_at_utc) >= $1::timestamptz
      AND COALESCE(latest_transition.transitioned_at_utc, o.updated_at_utc) <= $2::timestamptz
    ORDER BY o.order_id ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationRuntimeError {
    pub code: &'static str,
    pub message: String,
}

impl ReconciliationRuntimeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl Display for ReconciliationRuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ReconciliationRuntimeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationWindowRequest {
    pub run_id: String,
    pub window_started_at_utc: String,
    pub window_ended_at_utc: String,
    pub evaluated_at_utc: String,
    pub correlation_id: String,
}

impl ReconciliationWindowRequest {
    fn validate(&self) -> Result<(), ReconciliationRuntimeError> {
        validate_non_empty("run_id", &self.run_id)?;
        validate_non_empty("window_started_at_utc", &self.window_started_at_utc)?;
        validate_non_empty("window_ended_at_utc", &self.window_ended_at_utc)?;
        validate_non_empty("evaluated_at_utc", &self.evaluated_at_utc)?;
        validate_non_empty("correlation_id", &self.correlation_id)?;
        validate_timestamp_utc("window_started_at_utc", &self.window_started_at_utc)?;
        validate_timestamp_utc("window_ended_at_utc", &self.window_ended_at_utc)?;
        validate_timestamp_utc("evaluated_at_utc", &self.evaluated_at_utc)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconciliationExecutionOutcome {
    pub summary: ReconciliationRunSummary,
    pub diffs: Vec<ReconciliationDiffRecord>,
    pub latest_exposure_snapshot: ExposureSnapshot,
    pub block_new_order_creation: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconciliationIncidentView {
    pub run: ReconciliationRunSummary,
    pub diffs: Vec<ReconciliationDiffRecord>,
    pub latest_snapshot: Option<ExposureSnapshot>,
}

pub trait InternalTruthProvider: Send + Sync {
    fn load_internal_window<'a>(
        &'a self,
        request: &'a ReconciliationWindowRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ReconciliationOrderRecord>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    >;
}

pub trait VenueTruthProvider: Send + Sync {
    fn load_venue_window<'a>(
        &'a self,
        request: &'a ReconciliationWindowRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ReconciliationOrderRecord>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    >;
}

pub trait ReconciliationEvidenceStore: Send + Sync {
    fn persist_evidence<'a>(
        &'a self,
        run: &'a ReconciliationRunSummary,
        diffs: &'a [ReconciliationDiffRecord],
        snapshot: &'a ExposureSnapshot,
    ) -> Pin<Box<dyn Future<Output = Result<(), ReconciliationRuntimeError>> + Send + 'a>>;

    fn load_run<'a>(
        &'a self,
        run_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<Option<ReconciliationRunSummary>, ReconciliationRuntimeError>,
                > + Send
                + 'a,
        >,
    >;

    fn load_diffs<'a>(
        &'a self,
        run_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ReconciliationDiffRecord>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    >;

    fn load_latest_snapshot<'a>(
        &'a self,
        market_id: Option<&'a str>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<ExposureSnapshot>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    >;

    fn load_latest_snapshot_for_run<'a>(
        &'a self,
        run_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<ExposureSnapshot>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    >;
}

#[derive(Clone)]
pub struct PostgresInternalTruthProvider {
    pool: PgPool,
}

impl PostgresInternalTruthProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl InternalTruthProvider for PostgresInternalTruthProvider {
    fn load_internal_window<'a>(
        &'a self,
        request: &'a ReconciliationWindowRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ReconciliationOrderRecord>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let rows = sqlx::query(LOAD_INTERNAL_TRUTH_WINDOW_SQL)
                .bind(&request.window_started_at_utc)
                .bind(&request.window_ended_at_utc)
                .fetch_all(&self.pool)
                .await
                .map_err(|error| {
                    ReconciliationRuntimeError::new(
                        ReconciliationReasonCode::StateHydrationFailed.code(),
                        format!("load_internal_window failed: {error}"),
                    )
                })?;

            rows.into_iter()
                .map(|row| {
                    let lifecycle_state: String =
                        row.try_get("lifecycle_state").map_err(|error| {
                            ReconciliationRuntimeError::new(
                                ReconciliationReasonCode::StateHydrationFailed.code(),
                                format!("unable to decode `lifecycle_state`: {error}"),
                            )
                        })?;
                    let parsed_state =
                        OrderLifecycleState::parse(&lifecycle_state).map_err(|error| {
                            ReconciliationRuntimeError::new(
                                ReconciliationReasonCode::StateHydrationFailed.code(),
                                error.message,
                            )
                        })?;
                    Ok(ReconciliationOrderRecord {
                        order_id: row.try_get("order_id").map_err(|error| {
                            ReconciliationRuntimeError::new(
                                ReconciliationReasonCode::StateHydrationFailed.code(),
                                format!("unable to decode `order_id`: {error}"),
                            )
                        })?,
                        market_id: row.try_get("market_id").map_err(|error| {
                            ReconciliationRuntimeError::new(
                                ReconciliationReasonCode::StateHydrationFailed.code(),
                                format!("unable to decode `market_id`: {error}"),
                            )
                        })?,
                        lifecycle_state: parsed_state.as_str().to_string(),
                        quantity: None,
                        price: None,
                        observed_at_utc: row.try_get("observed_at_utc").map_err(|error| {
                            ReconciliationRuntimeError::new(
                                ReconciliationReasonCode::StateHydrationFailed.code(),
                                format!("unable to decode `observed_at_utc`: {error}"),
                            )
                        })?,
                        correlation_id: row.try_get("correlation_id").map_err(|error| {
                            ReconciliationRuntimeError::new(
                                ReconciliationReasonCode::StateHydrationFailed.code(),
                                format!("unable to decode `correlation_id`: {error}"),
                            )
                        })?,
                    })
                })
                .collect()
        })
    }
}

#[derive(Clone)]
pub struct StaticVenueTruthProvider {
    records: Arc<RwLock<Vec<ReconciliationOrderRecord>>>,
}

impl Default for StaticVenueTruthProvider {
    fn default() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl StaticVenueTruthProvider {
    pub fn set_records(&self, records: Vec<ReconciliationOrderRecord>) {
        *self
            .records
            .write()
            .expect("venue truth provider records should not be poisoned") = records;
    }
}

impl VenueTruthProvider for StaticVenueTruthProvider {
    fn load_venue_window<'a>(
        &'a self,
        _request: &'a ReconciliationWindowRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ReconciliationOrderRecord>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            Ok(self
                .records
                .read()
                .expect("venue truth provider records should not be poisoned")
                .clone())
        })
    }
}

#[derive(Clone)]
pub struct PostgresReconciliationEvidenceStore {
    pool: PgPool,
}

impl PostgresReconciliationEvidenceStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ReconciliationEvidenceStore for PostgresReconciliationEvidenceStore {
    fn persist_evidence<'a>(
        &'a self,
        run: &'a ReconciliationRunSummary,
        diffs: &'a [ReconciliationDiffRecord],
        snapshot: &'a ExposureSnapshot,
    ) -> Pin<Box<dyn Future<Output = Result<(), ReconciliationRuntimeError>> + Send + 'a>> {
        Box::pin(async move {
            persist_reconciliation_evidence(&self.pool, run, diffs, snapshot)
                .await
                .map_err(map_persistence_error)
        })
    }

    fn load_run<'a>(
        &'a self,
        run_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<Option<ReconciliationRunSummary>, ReconciliationRuntimeError>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            load_reconciliation_run(&self.pool, run_id)
                .await
                .map_err(map_persistence_error)
        })
    }

    fn load_diffs<'a>(
        &'a self,
        run_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ReconciliationDiffRecord>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            load_reconciliation_diffs(&self.pool, run_id)
                .await
                .map_err(map_persistence_error)
        })
    }

    fn load_latest_snapshot<'a>(
        &'a self,
        market_id: Option<&'a str>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<ExposureSnapshot>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            load_latest_exposure_snapshot(&self.pool, market_id)
                .await
                .map_err(map_persistence_error)
        })
    }

    fn load_latest_snapshot_for_run<'a>(
        &'a self,
        run_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<ExposureSnapshot>, ReconciliationRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            load_latest_exposure_snapshot_for_run(&self.pool, run_id)
                .await
                .map_err(map_persistence_error)
        })
    }
}

fn map_persistence_error(error: ReconciliationPersistenceError) -> ReconciliationRuntimeError {
    ReconciliationRuntimeError::new(error.code, error.to_string())
}

pub struct ReconciliationRuntime<I, V, S>
where
    I: InternalTruthProvider,
    V: VenueTruthProvider,
    S: ReconciliationEvidenceStore,
{
    internal_truth: I,
    venue_truth: V,
    evidence_store: S,
}

impl<I, V, S> ReconciliationRuntime<I, V, S>
where
    I: InternalTruthProvider,
    V: VenueTruthProvider,
    S: ReconciliationEvidenceStore,
{
    pub fn new(internal_truth: I, venue_truth: V, evidence_store: S) -> Self {
        Self {
            internal_truth,
            venue_truth,
            evidence_store,
        }
    }

    pub async fn execute_reconciliation(
        &self,
        request: ReconciliationWindowRequest,
    ) -> Result<ReconciliationExecutionOutcome, ReconciliationRuntimeError> {
        request.validate()?;
        emit_reconciliation_telemetry(ReconciliationTelemetryEvent {
            event_name: "execution_reconciliation_run_started_v1",
            run_id: &request.run_id,
            correlation_id: &request.correlation_id,
            reason_code: ReconciliationReasonCode::Matched.code(),
            mismatch_count: 0,
            mismatch_rate: 0.0,
            critical_halt: false,
            timestamp_utc: &request.evaluated_at_utc,
        });

        let internal_window = self.internal_truth.load_internal_window(&request).await?;
        let venue_window = self.venue_truth.load_venue_window(&request).await?;
        if internal_window.is_empty() && venue_window.is_empty() {
            return Err(ReconciliationRuntimeError::new(
                ReconciliationReasonCode::WindowUnavailable.code(),
                "internal and venue reconciliation windows are both empty",
            ));
        }

        let mut run_result = reconcile_window(
            &request.run_id,
            &request.window_started_at_utc,
            &request.window_ended_at_utc,
            &request.evaluated_at_utc,
            &request.correlation_id,
            &internal_window,
            &venue_window,
        )
        .map_err(map_contract_error)?;

        if run_result.summary.critical_halt {
            for diff in &mut run_result.diffs {
                diff.reason_code = ReconciliationReasonCode::CriticalMismatch
                    .code()
                    .to_string();
            }
        }

        let summary_reason = ReconciliationReasonCode::parse(&run_result.summary.reason_code)
            .map_err(map_contract_error)?;
        let snapshot = build_exposure_snapshot(
            &format!(
                "snapshot::{}::{}",
                normalize_identifier(&request.run_id),
                compact_timestamp_token(&request.evaluated_at_utc)
            ),
            &run_result.summary.run_id,
            None,
            &internal_window,
            summary_reason,
            &request.evaluated_at_utc,
            &request.correlation_id,
        )
        .map_err(map_contract_error)?;

        self.evidence_store
            .persist_evidence(&run_result.summary, &run_result.diffs, &snapshot)
            .await?;

        emit_reconciliation_telemetry(ReconciliationTelemetryEvent {
            event_name: "execution_reconciliation_run_completed_v1",
            run_id: &request.run_id,
            correlation_id: &request.correlation_id,
            reason_code: &run_result.summary.reason_code,
            mismatch_count: run_result.summary.mismatch_count,
            mismatch_rate: run_result.summary.mismatch_rate,
            critical_halt: run_result.summary.critical_halt,
            timestamp_utc: &request.evaluated_at_utc,
        });

        Ok(ReconciliationExecutionOutcome {
            block_new_order_creation: run_result.summary.critical_halt,
            summary: run_result.summary,
            diffs: run_result.diffs,
            latest_exposure_snapshot: snapshot,
        })
    }

    pub async fn latest_exposure_snapshot(
        &self,
        actor_role: &str,
        market_id: Option<&str>,
    ) -> Result<Option<ExposureSnapshot>, ReconciliationRuntimeError> {
        authorize_reconciliation_read(actor_role).map_err(map_contract_error)?;
        self.evidence_store.load_latest_snapshot(market_id).await
    }

    pub async fn incident_evidence(
        &self,
        actor_role: &str,
        run_id: &str,
    ) -> Result<Option<ReconciliationIncidentView>, ReconciliationRuntimeError> {
        authorize_reconciliation_read(actor_role).map_err(map_contract_error)?;
        let normalized_run_id = normalize_identifier(run_id);
        let Some(run) = self.evidence_store.load_run(&normalized_run_id).await? else {
            return Ok(None);
        };
        let diffs = self.evidence_store.load_diffs(&normalized_run_id).await?;
        let latest_snapshot = self
            .evidence_store
            .load_latest_snapshot_for_run(&normalized_run_id)
            .await?;
        Ok(Some(ReconciliationIncidentView {
            run,
            diffs,
            latest_snapshot,
        }))
    }
}

fn map_contract_error(error: ReconciliationContractError) -> ReconciliationRuntimeError {
    ReconciliationRuntimeError::new(error.code, error.message)
}

fn normalize_identifier(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), ReconciliationRuntimeError> {
    if value.trim().is_empty() {
        return Err(ReconciliationRuntimeError::new(
            ReconciliationReasonCode::InvalidPayload.code(),
            format!("{field} cannot be blank"),
        ));
    }
    Ok(())
}

fn validate_timestamp_utc(
    field: &'static str,
    value: &str,
) -> Result<(), ReconciliationRuntimeError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
        ReconciliationRuntimeError::new(
            ReconciliationReasonCode::InvalidPayload.code(),
            format!("{field} must be an RFC3339 UTC timestamp: {error}"),
        )
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(ReconciliationRuntimeError::new(
            ReconciliationReasonCode::InvalidPayload.code(),
            format!("{field} must use UTC `Z` offset"),
        ));
    }
    Ok(())
}

fn compact_timestamp_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

fn emit_reconciliation_telemetry(event: ReconciliationTelemetryEvent<'_>) {
    println!(
        "{}",
        serde_json::to_string(&event).expect("reconciliation telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct ReconciliationTelemetryEvent<'a> {
    event_name: &'a str,
    run_id: &'a str,
    correlation_id: &'a str,
    reason_code: &'a str,
    mismatch_count: i64,
    mismatch_rate: f64,
    critical_halt: bool,
    timestamp_utc: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    #[derive(Clone)]
    struct StaticInternalTruthProvider {
        records: Vec<ReconciliationOrderRecord>,
    }

    impl InternalTruthProvider for StaticInternalTruthProvider {
        fn load_internal_window<'a>(
            &'a self,
            _request: &'a ReconciliationWindowRequest,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<Vec<ReconciliationOrderRecord>, ReconciliationRuntimeError>,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move { Ok(self.records.clone()) })
        }
    }

    #[derive(Debug, Default)]
    struct InMemoryEvidenceState {
        runs: BTreeMap<String, ReconciliationRunSummary>,
        diffs: BTreeMap<String, Vec<ReconciliationDiffRecord>>,
        snapshots: Vec<ExposureSnapshot>,
    }

    #[derive(Debug, Default, Clone)]
    struct InMemoryEvidenceStore {
        state: Arc<Mutex<InMemoryEvidenceState>>,
    }

    impl ReconciliationEvidenceStore for InMemoryEvidenceStore {
        fn persist_evidence<'a>(
            &'a self,
            run: &'a ReconciliationRunSummary,
            diffs: &'a [ReconciliationDiffRecord],
            snapshot: &'a ExposureSnapshot,
        ) -> Pin<Box<dyn Future<Output = Result<(), ReconciliationRuntimeError>> + Send + 'a>>
        {
            Box::pin(async move {
                let mut state = self.state.lock().map_err(|_| {
                    ReconciliationRuntimeError::new(
                        ReconciliationReasonCode::PersistenceUnavailable.code(),
                        "in-memory reconciliation evidence store is poisoned",
                    )
                })?;
                state.runs.insert(run.run_id.clone(), run.clone());
                state.diffs.insert(run.run_id.clone(), diffs.to_vec());
                state.snapshots.push(snapshot.clone());
                Ok(())
            })
        }

        fn load_run<'a>(
            &'a self,
            run_id: &'a str,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            Option<ReconciliationRunSummary>,
                            ReconciliationRuntimeError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                let state = self.state.lock().map_err(|_| {
                    ReconciliationRuntimeError::new(
                        ReconciliationReasonCode::PersistenceUnavailable.code(),
                        "in-memory reconciliation evidence store is poisoned",
                    )
                })?;
                Ok(state.runs.get(run_id).cloned())
            })
        }

        fn load_diffs<'a>(
            &'a self,
            run_id: &'a str,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<Vec<ReconciliationDiffRecord>, ReconciliationRuntimeError>,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                let state = self.state.lock().map_err(|_| {
                    ReconciliationRuntimeError::new(
                        ReconciliationReasonCode::PersistenceUnavailable.code(),
                        "in-memory reconciliation evidence store is poisoned",
                    )
                })?;
                Ok(state.diffs.get(run_id).cloned().unwrap_or_default())
            })
        }

        fn load_latest_snapshot<'a>(
            &'a self,
            market_id: Option<&'a str>,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Option<ExposureSnapshot>, ReconciliationRuntimeError>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                let state = self.state.lock().map_err(|_| {
                    ReconciliationRuntimeError::new(
                        ReconciliationReasonCode::PersistenceUnavailable.code(),
                        "in-memory reconciliation evidence store is poisoned",
                    )
                })?;
                let snapshot = state
                    .snapshots
                    .iter()
                    .filter(|candidate| {
                        market_id
                            .map(|value| candidate.market_id.as_deref() == Some(value))
                            .unwrap_or(candidate.market_id.is_none())
                    })
                    .max_by(|left, right| {
                        left.captured_at_utc
                            .cmp(&right.captured_at_utc)
                            .then_with(|| left.snapshot_id.cmp(&right.snapshot_id))
                    })
                    .cloned();
                Ok(snapshot)
            })
        }

        fn load_latest_snapshot_for_run<'a>(
            &'a self,
            run_id: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Option<ExposureSnapshot>, ReconciliationRuntimeError>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                let state = self.state.lock().map_err(|_| {
                    ReconciliationRuntimeError::new(
                        ReconciliationReasonCode::PersistenceUnavailable.code(),
                        "in-memory reconciliation evidence store is poisoned",
                    )
                })?;
                let normalized_run_id = normalize_identifier(run_id);
                let snapshot = state
                    .snapshots
                    .iter()
                    .filter(|candidate| candidate.run_id == normalized_run_id)
                    .max_by(|left, right| {
                        left.captured_at_utc
                            .cmp(&right.captured_at_utc)
                            .then_with(|| left.snapshot_id.cmp(&right.snapshot_id))
                    })
                    .cloned();
                Ok(snapshot)
            })
        }
    }

    fn sample_record(order_id: &str, state: &str) -> ReconciliationOrderRecord {
        ReconciliationOrderRecord {
            order_id: order_id.to_string(),
            market_id: "market-1".to_string(),
            lifecycle_state: state.to_string(),
            quantity: Some(1.0),
            price: Some(0.45),
            observed_at_utc: "2026-04-06T00:00:30Z".to_string(),
            correlation_id: "corr-1".to_string(),
        }
    }

    fn sample_request(run_id: &str, correlation_id: &str) -> ReconciliationWindowRequest {
        ReconciliationWindowRequest {
            run_id: run_id.to_string(),
            window_started_at_utc: "2026-04-06T00:00:00Z".to_string(),
            window_ended_at_utc: "2026-04-06T00:01:00Z".to_string(),
            evaluated_at_utc: "2026-04-06T00:01:01Z".to_string(),
            correlation_id: correlation_id.to_string(),
        }
    }

    #[tokio::test]
    async fn execution_run_is_deterministic_for_identical_inputs() {
        let internal = StaticInternalTruthProvider {
            records: vec![
                sample_record("order-1", "live"),
                sample_record("order-2", "live"),
            ],
        };
        let venue = StaticVenueTruthProvider::default();
        venue.set_records(vec![
            sample_record("order-1", "live"),
            sample_record("order-2", "filled"),
        ]);

        let runtime_a = ReconciliationRuntime::new(
            internal.clone(),
            venue.clone(),
            InMemoryEvidenceStore::default(),
        );
        let runtime_b =
            ReconciliationRuntime::new(internal, venue, InMemoryEvidenceStore::default());
        let first = runtime_a
            .execute_reconciliation(sample_request("run-a", "corr-a"))
            .await
            .expect("first reconciliation run should succeed");
        let second = runtime_b
            .execute_reconciliation(sample_request("run-a", "corr-a"))
            .await
            .expect("second reconciliation run should succeed");

        assert_eq!(first.summary, second.summary);
        assert_eq!(first.diffs, second.diffs);
        assert_eq!(first.diffs.len(), 1);
        assert_eq!(first.diffs[0].order_id, "order-2");
    }

    #[tokio::test]
    async fn critical_mismatch_run_enforces_safe_state_and_keeps_snapshot_readable() {
        let internal = StaticInternalTruthProvider {
            records: vec![
                sample_record("order-1", "live"),
                sample_record("order-2", "live"),
                sample_record("order-3", "live"),
                sample_record("order-4", "live"),
            ],
        };
        let venue = StaticVenueTruthProvider::default();
        venue.set_records(vec![
            sample_record("order-1", "live"),
            sample_record("order-2", "filled"),
        ]);
        let store = InMemoryEvidenceStore::default();
        let runtime = ReconciliationRuntime::new(internal, venue, store);

        let outcome = runtime
            .execute_reconciliation(sample_request("run-critical", "corr-critical"))
            .await
            .expect("critical mismatch run should complete");
        assert!(outcome.block_new_order_creation);
        assert_eq!(
            outcome.summary.reason_code,
            ReconciliationReasonCode::CriticalMismatch.code()
        );
        assert!(
            outcome
                .diffs
                .iter()
                .all(|diff| diff.reason_code == ReconciliationReasonCode::CriticalMismatch.code())
        );

        let snapshot = runtime
            .latest_exposure_snapshot("operational_control", None)
            .await
            .expect("snapshot read should succeed while halted")
            .expect("snapshot should exist");
        assert_eq!(snapshot.run_id, "run-critical");
        assert_eq!(snapshot.captured_at_utc, "2026-04-06T00:01:01Z");
    }

    #[tokio::test]
    async fn unauthorized_read_queries_fail_closed_with_machine_reason_code() {
        let internal = StaticInternalTruthProvider {
            records: vec![sample_record("order-1", "live")],
        };
        let venue = StaticVenueTruthProvider::default();
        venue.set_records(vec![sample_record("order-1", "live")]);
        let runtime = ReconciliationRuntime::new(internal, venue, InMemoryEvidenceStore::default());
        runtime
            .execute_reconciliation(sample_request("run-authz", "corr-authz"))
            .await
            .expect("run should succeed");

        let snapshot_error = runtime
            .latest_exposure_snapshot("guest", None)
            .await
            .expect_err("unauthorized role should not read snapshot");
        assert_eq!(
            snapshot_error.code,
            ReconciliationReasonCode::Unauthorized.code()
        );

        let incident_error = runtime
            .incident_evidence("guest", "run-authz")
            .await
            .expect_err("unauthorized role should not read incident evidence");
        assert_eq!(
            incident_error.code,
            ReconciliationReasonCode::Unauthorized.code()
        );
    }

    #[tokio::test]
    async fn incident_query_path_satisfies_operability_target() {
        let mut internal_records = Vec::new();
        let mut venue_records = Vec::new();
        for index in 0..2000 {
            internal_records.push(sample_record(&format!("order-{index}"), "live"));
            venue_records.push(sample_record(&format!("order-{index}"), "live"));
        }
        venue_records[0].lifecycle_state = "filled".to_string();
        venue_records[1].lifecycle_state = "filled".to_string();

        let runtime = ReconciliationRuntime::new(
            StaticInternalTruthProvider {
                records: internal_records,
            },
            {
                let provider = StaticVenueTruthProvider::default();
                provider.set_records(venue_records);
                provider
            },
            InMemoryEvidenceStore::default(),
        );
        runtime
            .execute_reconciliation(sample_request("run-incident", "corr-incident"))
            .await
            .expect("run should succeed");

        let started = Instant::now();
        let incident = runtime
            .incident_evidence("read_only_analytics", "run-incident")
            .await
            .expect("incident query should succeed")
            .expect("incident evidence should exist");
        let elapsed = started.elapsed();

        assert_eq!(incident.run.run_id, "run-incident");
        assert!(incident.diffs.len() >= 2);
        assert!(elapsed <= Duration::from_secs(5));
    }

    #[tokio::test]
    async fn incident_evidence_returns_snapshot_for_requested_run() {
        let internal = StaticInternalTruthProvider {
            records: vec![sample_record("order-1", "live")],
        };
        let venue = {
            let provider = StaticVenueTruthProvider::default();
            provider.set_records(vec![sample_record("order-1", "live")]);
            provider
        };
        let store = InMemoryEvidenceStore::default();
        let runtime = ReconciliationRuntime::new(internal, venue, store);

        let mut first = sample_request("run-one", "corr-one");
        first.evaluated_at_utc = "2026-04-06T00:01:01Z".to_string();
        runtime
            .execute_reconciliation(first)
            .await
            .expect("first run should succeed");

        let mut second = sample_request("run-two", "corr-two");
        second.evaluated_at_utc = "2026-04-06T00:02:01Z".to_string();
        runtime
            .execute_reconciliation(second)
            .await
            .expect("second run should succeed");

        let incident_one = runtime
            .incident_evidence("read_only_analytics", "run-one")
            .await
            .expect("incident evidence query should succeed")
            .expect("incident evidence should exist");

        assert_eq!(incident_one.run.run_id, "run-one");
        assert_eq!(
            incident_one
                .latest_snapshot
                .as_ref()
                .expect("snapshot should exist")
                .run_id,
            "run-one"
        );
    }
}
