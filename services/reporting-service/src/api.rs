use crate::contracts::artifacts::{
    self, changelog_artifact_for, compute_record_checksum, contract_key_for_dataset,
    expected_changelog_checksum, expected_schema_checksum, schema_artifact_for,
};
use crate::contracts::lifecycle::{
    ContractLifecycleStatus, ReportingContractVersionDescriptor, validate_contract_lifecycle,
};
use crate::read_models::queries::{
    ReportingDatasetResponse, ReportingReadModelError, ReportingReadModelOrchestrator,
    ReportingReadRequest,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use domain::reporting::{
    DEFAULT_REPORTING_LIMIT, PerformanceReportingRow, PositionReportingRow, ReportingContractError,
    ReportingValidationIssue, RiskEventReportingRow, TradeReportingRow, authorize_reporting_read,
    parse_utc_timestamp,
};
use persistence::postgres::api_contract_versions::{
    ApiContractVersionPersistenceError, ApiContractVersionRecord, ApiContractVersionUpsert,
    list_api_contract_versions, load_active_api_contract_version, load_api_contract_version,
    upsert_api_contract_version,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

type AsyncResult<T> = Pin<Box<dyn Future<Output = T> + Send>>;

#[derive(Clone)]
pub struct ReportingApiState {
    query_port: Arc<dyn ReportingQueryPort>,
    contract_port: Arc<dyn ContractVersionPort>,
}

impl ReportingApiState {
    pub fn new(
        query_port: Arc<dyn ReportingQueryPort>,
        contract_port: Arc<dyn ContractVersionPort>,
    ) -> Self {
        Self {
            query_port,
            contract_port,
        }
    }
}

pub fn reporting_router(state: ReportingApiState) -> Router {
    Router::new()
        .route("/api/v1/reporting/trades", get(read_trades))
        .route("/api/v1/reporting/positions", get(read_positions))
        .route("/api/v1/reporting/risk-events", get(read_risk_events))
        .route("/api/v1/reporting/performance", get(read_performance))
        .route(
            "/api/v1/reporting/alpha-attribution",
            get(read_alpha_attribution),
        )
        .route(
            "/api/v1/reporting/contracts/{dataset}/versions",
            get(list_contract_versions),
        )
        .route(
            "/api/v1/reporting/contracts/{dataset}/versions/{contract_version}",
            get(read_contract_version_metadata),
        )
        .route(
            "/api/v1/reporting/contracts/{dataset}/versions/{contract_version}/schema",
            get(read_contract_schema_artifact),
        )
        .route(
            "/api/v1/reporting/contracts/{dataset}/versions/{contract_version}/changelog",
            get(read_contract_changelog_artifact),
        )
        .with_state(state)
}

pub trait ReportingQueryPort: Send + Sync {
    fn query_trade_rows(
        &self,
        request: ReportingReadRequest,
    ) -> AsyncResult<Result<ReportingDatasetResponse<TradeReportingRow>, ReportingReadModelError>>;

    fn query_position_rows(
        &self,
        request: ReportingReadRequest,
    ) -> AsyncResult<Result<ReportingDatasetResponse<PositionReportingRow>, ReportingReadModelError>>;

    fn query_risk_event_rows(
        &self,
        request: ReportingReadRequest,
    ) -> AsyncResult<Result<ReportingDatasetResponse<RiskEventReportingRow>, ReportingReadModelError>>;

    fn query_performance_rows(
        &self,
        request: ReportingReadRequest,
    ) -> AsyncResult<
        Result<ReportingDatasetResponse<PerformanceReportingRow>, ReportingReadModelError>,
    >;
}

impl ReportingQueryPort for ReportingReadModelOrchestrator {
    fn query_trade_rows(
        &self,
        request: ReportingReadRequest,
    ) -> AsyncResult<Result<ReportingDatasetResponse<TradeReportingRow>, ReportingReadModelError>>
    {
        let orchestrator = self.clone();
        Box::pin(async move { orchestrator.query_trade_rows(&request).await })
    }

    fn query_position_rows(
        &self,
        request: ReportingReadRequest,
    ) -> AsyncResult<Result<ReportingDatasetResponse<PositionReportingRow>, ReportingReadModelError>>
    {
        let orchestrator = self.clone();
        Box::pin(async move { orchestrator.query_position_rows(&request).await })
    }

    fn query_risk_event_rows(
        &self,
        request: ReportingReadRequest,
    ) -> AsyncResult<Result<ReportingDatasetResponse<RiskEventReportingRow>, ReportingReadModelError>>
    {
        let orchestrator = self.clone();
        Box::pin(async move { orchestrator.query_risk_event_rows(&request).await })
    }

    fn query_performance_rows(
        &self,
        request: ReportingReadRequest,
    ) -> AsyncResult<
        Result<ReportingDatasetResponse<PerformanceReportingRow>, ReportingReadModelError>,
    > {
        let orchestrator = self.clone();
        Box::pin(async move { orchestrator.query_performance_rows(&request).await })
    }
}

pub trait ContractVersionPort: Send + Sync {
    fn resolve_contract_version(
        &self,
        contract_key: String,
        requested_version: Option<String>,
        as_of_utc: String,
    ) -> AsyncResult<Result<ReportingContractVersionDescriptor, ReportingReadModelError>>;

    fn list_contract_versions(
        &self,
        contract_key: String,
    ) -> AsyncResult<Result<Vec<ReportingContractVersionDescriptor>, ReportingReadModelError>>;
}

#[derive(Clone)]
pub struct PostgresContractVersionPort {
    pool: PgPool,
}

impl PostgresContractVersionPort {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_contract_version_internal(
        &self,
        upsert: &ApiContractVersionUpsert,
    ) -> Result<ReportingContractVersionDescriptor, ReportingReadModelError> {
        let record = upsert_api_contract_version(&self.pool, upsert)
            .await
            .map_err(map_persistence_error)?;
        let replacement_release = load_replacement_release(
            &self.pool,
            &record.contract_key,
            record.replacement_contract_version.as_deref(),
        )
        .await?;
        build_descriptor(record, replacement_release.as_deref())
    }
}

impl ContractVersionPort for PostgresContractVersionPort {
    fn resolve_contract_version(
        &self,
        contract_key: String,
        requested_version: Option<String>,
        as_of_utc: String,
    ) -> AsyncResult<Result<ReportingContractVersionDescriptor, ReportingReadModelError>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let record = match requested_version.as_deref() {
                Some(version) => load_api_contract_version(&pool, &contract_key, version)
                    .await
                    .map_err(map_persistence_error)?
                    .ok_or_else(|| {
                        invalid_payload_error_with_field(
                            "unknown contract_version for dataset",
                            "contract_version",
                        )
                    })?,
                None => load_active_api_contract_version(&pool, &contract_key, &as_of_utc)
                    .await
                    .map_err(map_persistence_error)?
                    .ok_or_else(|| {
                        ReportingReadModelError::dependency_unavailable(
                            "no active contract version metadata found",
                        )
                    })?,
            };

            let replacement_release = load_replacement_release(
                &pool,
                &record.contract_key,
                record.replacement_contract_version.as_deref(),
            )
            .await?;
            let descriptor = build_descriptor(record, replacement_release.as_deref())?;
            ensure_contract_support_window(&descriptor, &as_of_utc)?;
            Ok(descriptor)
        })
    }

    fn list_contract_versions(
        &self,
        contract_key: String,
    ) -> AsyncResult<Result<Vec<ReportingContractVersionDescriptor>, ReportingReadModelError>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let records = list_api_contract_versions(&pool, &contract_key)
                .await
                .map_err(map_persistence_error)?;
            let release_by_version: BTreeMap<String, String> = records
                .iter()
                .map(|record| {
                    (
                        record.contract_version.clone(),
                        record.release_at_utc.clone(),
                    )
                })
                .collect();

            let mut descriptors = Vec::with_capacity(records.len());
            for record in records {
                let replacement_release = record
                    .replacement_contract_version
                    .as_ref()
                    .and_then(|version| release_by_version.get(version))
                    .cloned();
                descriptors.push(build_descriptor(record, replacement_release.as_deref())?);
            }
            Ok(descriptors)
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct UnavailableContractVersionPort;

impl ContractVersionPort for UnavailableContractVersionPort {
    fn resolve_contract_version(
        &self,
        _contract_key: String,
        _requested_version: Option<String>,
        _as_of_utc: String,
    ) -> AsyncResult<Result<ReportingContractVersionDescriptor, ReportingReadModelError>> {
        Box::pin(async {
            Err(ReportingReadModelError::dependency_unavailable(
                "contract version persistence adapter is unavailable",
            ))
        })
    }

    fn list_contract_versions(
        &self,
        _contract_key: String,
    ) -> AsyncResult<Result<Vec<ReportingContractVersionDescriptor>, ReportingReadModelError>> {
        Box::pin(async {
            Err(ReportingReadModelError::dependency_unavailable(
                "contract version persistence adapter is unavailable",
            ))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReportingDatasetRoute {
    Trades,
    Positions,
    RiskEvents,
    Performance,
    AlphaAttribution,
}

impl ReportingDatasetRoute {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Trades => "trades",
            Self::Positions => "positions",
            Self::RiskEvents => "risk-events",
            Self::Performance => "performance",
            Self::AlphaAttribution => "alpha-attribution",
        }
    }

    const fn endpoint_path(self) -> &'static str {
        match self {
            Self::Trades => "/api/v1/reporting/trades",
            Self::Positions => "/api/v1/reporting/positions",
            Self::RiskEvents => "/api/v1/reporting/risk-events",
            Self::Performance => "/api/v1/reporting/performance",
            Self::AlphaAttribution => "/api/v1/reporting/alpha-attribution",
        }
    }

    const fn contract_key(self) -> &'static str {
        match self {
            Self::Trades => artifacts::CONTRACT_KEY_TRADES,
            Self::Positions => artifacts::CONTRACT_KEY_POSITIONS,
            Self::RiskEvents => artifacts::CONTRACT_KEY_RISK_EVENTS,
            Self::Performance => artifacts::CONTRACT_KEY_PERFORMANCE,
            Self::AlphaAttribution => artifacts::CONTRACT_KEY_ALPHA_ATTRIBUTION,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ReportingDatasetQuery {
    start_inclusive_utc: Option<String>,
    end_exclusive_utc: Option<String>,
    market_id: Option<String>,
    alpha_id: Option<String>,
    correlation_id: Option<String>,
    limit: Option<String>,
    contract_version: Option<String>,
}

#[derive(Debug, Clone)]
struct RequestContext {
    actor_id: String,
    actor_role: String,
    correlation_id: String,
    timestamp_utc: String,
}

#[derive(Debug, Clone)]
struct DatasetQueryResult {
    rows: Value,
    as_of_utc: String,
    source: String,
    reason_code: String,
    correlation_id: String,
    start_inclusive_utc: String,
    end_exclusive_utc: String,
}

#[derive(Debug, Serialize)]
struct ReportingApiEnvelope {
    data: Value,
    meta: ReportingApiMeta,
    error: Value,
}

#[derive(Debug, Serialize)]
struct ReportingApiMeta {
    dataset: String,
    contract_key: String,
    contract_version: String,
    as_of_utc: String,
    source: String,
    reason_code: String,
    correlation_id: String,
    endpoint: String,
    actor_id: String,
    actor_role: String,
    timestamp_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_inclusive_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_exclusive_utc: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReportingContractTelemetryEvent {
    event_name: &'static str,
    actor_id: String,
    actor_role: String,
    endpoint: String,
    dataset: String,
    contract_version: String,
    reason_code: String,
    correlation_id: String,
    timestamp_utc: String,
}

async fn read_trades(
    State(state): State<ReportingApiState>,
    headers: HeaderMap,
    Query(query): Query<ReportingDatasetQuery>,
) -> Response {
    handle_dataset_read(state, headers, query, ReportingDatasetRoute::Trades).await
}

async fn read_positions(
    State(state): State<ReportingApiState>,
    headers: HeaderMap,
    Query(query): Query<ReportingDatasetQuery>,
) -> Response {
    handle_dataset_read(state, headers, query, ReportingDatasetRoute::Positions).await
}

async fn read_risk_events(
    State(state): State<ReportingApiState>,
    headers: HeaderMap,
    Query(query): Query<ReportingDatasetQuery>,
) -> Response {
    handle_dataset_read(state, headers, query, ReportingDatasetRoute::RiskEvents).await
}

async fn read_performance(
    State(state): State<ReportingApiState>,
    headers: HeaderMap,
    Query(query): Query<ReportingDatasetQuery>,
) -> Response {
    handle_dataset_read(state, headers, query, ReportingDatasetRoute::Performance).await
}

async fn read_alpha_attribution(
    State(state): State<ReportingApiState>,
    headers: HeaderMap,
    Query(query): Query<ReportingDatasetQuery>,
) -> Response {
    handle_dataset_read(
        state,
        headers,
        query,
        ReportingDatasetRoute::AlphaAttribution,
    )
    .await
}

async fn list_contract_versions(
    State(state): State<ReportingApiState>,
    headers: HeaderMap,
    Path(dataset): Path<String>,
) -> Response {
    let Some(contract_key) = contract_key_for_dataset(&dataset) else {
        return error_envelope_response(
            StatusCode::BAD_REQUEST,
            dataset,
            "unknown".to_string(),
            "unknown".to_string(),
            "/api/v1/reporting/contracts/{dataset}/versions".to_string(),
            RequestContext {
                actor_id: "unknown".to_string(),
                actor_role: "unknown".to_string(),
                correlation_id: "reporting-contracts-invalid-dataset".to_string(),
                timestamp_utc: now_utc_rfc3339(),
            },
            invalid_payload_error_with_field("unsupported dataset path", "dataset"),
        );
    };
    let context =
        match extract_request_context(&headers, None, &format!("reporting-contracts-{dataset}")) {
            Ok(context) => context,
            Err(error) => {
                return error_envelope_response(
                    status_for_reason_code(error.code),
                    dataset,
                    contract_key.to_string(),
                    "unknown".to_string(),
                    "/api/v1/reporting/contracts/{dataset}/versions".to_string(),
                    RequestContext {
                        actor_id: "unknown".to_string(),
                        actor_role: "unknown".to_string(),
                        correlation_id: "reporting-contracts-auth-failure".to_string(),
                        timestamp_utc: now_utc_rfc3339(),
                    },
                    error,
                );
            }
        };

    let versions = match state
        .contract_port
        .list_contract_versions(contract_key.to_string())
        .await
    {
        Ok(descriptors) => descriptors
            .into_iter()
            .map(|descriptor| json!(ContractVersionMetadata::from_descriptor(descriptor)))
            .collect::<Vec<_>>(),
        Err(error) => {
            return error_envelope_response(
                status_for_reason_code(error.code),
                dataset,
                contract_key.to_string(),
                "unknown".to_string(),
                "/api/v1/reporting/contracts/{dataset}/versions".to_string(),
                context,
                error,
            );
        }
    };

    let envelope = ReportingApiEnvelope {
        data: json!({ "versions": versions }),
        meta: ReportingApiMeta {
            dataset: dataset.to_string(),
            contract_key: contract_key.to_string(),
            contract_version: "multi".to_string(),
            as_of_utc: now_utc_rfc3339(),
            source: "reporting_service.contracts.v1".to_string(),
            reason_code: "reporting_ready".to_string(),
            correlation_id: context.correlation_id.clone(),
            endpoint: "/api/v1/reporting/contracts/{dataset}/versions".to_string(),
            actor_id: context.actor_id.clone(),
            actor_role: context.actor_role.clone(),
            timestamp_utc: context.timestamp_utc.clone(),
            start_inclusive_utc: None,
            end_exclusive_utc: None,
        },
        error: Value::Null,
    };
    emit_telemetry(
        &context,
        "/api/v1/reporting/contracts/{dataset}/versions",
        &dataset,
        "multi",
        "reporting_ready",
    );
    (StatusCode::OK, Json(envelope)).into_response()
}

async fn read_contract_version_metadata(
    State(state): State<ReportingApiState>,
    headers: HeaderMap,
    Path((dataset, contract_version)): Path<(String, String)>,
) -> Response {
    read_contract_version_resource(
        state,
        headers,
        dataset,
        contract_version,
        ContractVersionResource::Metadata,
    )
    .await
}

async fn read_contract_schema_artifact(
    State(state): State<ReportingApiState>,
    headers: HeaderMap,
    Path((dataset, contract_version)): Path<(String, String)>,
) -> Response {
    read_contract_version_resource(
        state,
        headers,
        dataset,
        contract_version,
        ContractVersionResource::Schema,
    )
    .await
}

async fn read_contract_changelog_artifact(
    State(state): State<ReportingApiState>,
    headers: HeaderMap,
    Path((dataset, contract_version)): Path<(String, String)>,
) -> Response {
    read_contract_version_resource(
        state,
        headers,
        dataset,
        contract_version,
        ContractVersionResource::Changelog,
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContractVersionResource {
    Metadata,
    Schema,
    Changelog,
}

impl ContractVersionResource {
    const fn endpoint_path(self) -> &'static str {
        match self {
            Self::Metadata => "/api/v1/reporting/contracts/{dataset}/versions/{contract_version}",
            Self::Schema => {
                "/api/v1/reporting/contracts/{dataset}/versions/{contract_version}/schema"
            }
            Self::Changelog => {
                "/api/v1/reporting/contracts/{dataset}/versions/{contract_version}/changelog"
            }
        }
    }
}

async fn read_contract_version_resource(
    state: ReportingApiState,
    headers: HeaderMap,
    dataset: String,
    contract_version: String,
    resource: ContractVersionResource,
) -> Response {
    let Some(contract_key) = contract_key_for_dataset(&dataset) else {
        return error_envelope_response(
            StatusCode::BAD_REQUEST,
            dataset,
            "unknown".to_string(),
            contract_version,
            resource.endpoint_path().to_string(),
            RequestContext {
                actor_id: "unknown".to_string(),
                actor_role: "unknown".to_string(),
                correlation_id: "reporting-contracts-invalid-dataset".to_string(),
                timestamp_utc: now_utc_rfc3339(),
            },
            invalid_payload_error_with_field("unsupported dataset path", "dataset"),
        );
    };

    let context = match extract_request_context(
        &headers,
        None,
        &format!("reporting-contracts-{dataset}-{contract_version}"),
    ) {
        Ok(context) => context,
        Err(error) => {
            return error_envelope_response(
                status_for_reason_code(error.code),
                dataset,
                contract_key.to_string(),
                contract_version,
                resource.endpoint_path().to_string(),
                RequestContext {
                    actor_id: "unknown".to_string(),
                    actor_role: "unknown".to_string(),
                    correlation_id: "reporting-contracts-auth-failure".to_string(),
                    timestamp_utc: now_utc_rfc3339(),
                },
                error,
            );
        }
    };

    let as_of_utc = now_utc_rfc3339();
    let descriptor = match state
        .contract_port
        .resolve_contract_version(
            contract_key.to_string(),
            Some(contract_version.clone()),
            as_of_utc.clone(),
        )
        .await
    {
        Ok(descriptor) => descriptor,
        Err(error) => {
            return error_envelope_response(
                status_for_reason_code(error.code),
                dataset,
                contract_key.to_string(),
                contract_version,
                resource.endpoint_path().to_string(),
                context,
                error,
            );
        }
    };

    let data = match resource {
        ContractVersionResource::Metadata => {
            json!(ContractVersionMetadata::from_descriptor(descriptor.clone()))
        }
        ContractVersionResource::Schema => {
            let Some(artifact) =
                schema_artifact_for(&descriptor.contract_key, &descriptor.contract_version)
            else {
                return error_envelope_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    dataset,
                    descriptor.contract_key.clone(),
                    descriptor.contract_version.clone(),
                    resource.endpoint_path().to_string(),
                    context,
                    ReportingReadModelError::dependency_unavailable(
                        "schema artifact payload is unavailable",
                    ),
                );
            };
            json!(artifact)
        }
        ContractVersionResource::Changelog => {
            let Some(artifact) = changelog_artifact_for(&descriptor.contract_version) else {
                return error_envelope_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    dataset,
                    descriptor.contract_key.clone(),
                    descriptor.contract_version.clone(),
                    resource.endpoint_path().to_string(),
                    context,
                    ReportingReadModelError::dependency_unavailable(
                        "changelog artifact payload is unavailable",
                    ),
                );
            };
            json!(artifact)
        }
    };

    let reason_code = "reporting_ready";
    let envelope = ReportingApiEnvelope {
        data,
        meta: ReportingApiMeta {
            dataset: dataset.clone(),
            contract_key: descriptor.contract_key.clone(),
            contract_version: descriptor.contract_version.clone(),
            as_of_utc: now_utc_rfc3339(),
            source: "reporting_service.contracts.v1".to_string(),
            reason_code: reason_code.to_string(),
            correlation_id: context.correlation_id.clone(),
            endpoint: resource.endpoint_path().to_string(),
            actor_id: context.actor_id.clone(),
            actor_role: context.actor_role.clone(),
            timestamp_utc: context.timestamp_utc.clone(),
            start_inclusive_utc: None,
            end_exclusive_utc: None,
        },
        error: Value::Null,
    };
    emit_telemetry(
        &context,
        resource.endpoint_path(),
        &dataset,
        &descriptor.contract_version,
        reason_code,
    );
    (StatusCode::OK, Json(envelope)).into_response()
}

async fn handle_dataset_read(
    state: ReportingApiState,
    headers: HeaderMap,
    query: ReportingDatasetQuery,
    dataset: ReportingDatasetRoute,
) -> Response {
    let context = match extract_request_context(
        &headers,
        query.correlation_id.as_deref(),
        dataset.as_str(),
    ) {
        Ok(context) => context,
        Err(error) => {
            return error_envelope_response(
                status_for_reason_code(error.code),
                dataset.as_str().to_string(),
                dataset.contract_key().to_string(),
                "unknown".to_string(),
                dataset.endpoint_path().to_string(),
                RequestContext {
                    actor_id: "unknown".to_string(),
                    actor_role: "unknown".to_string(),
                    correlation_id: query
                        .correlation_id
                        .unwrap_or_else(|| "reporting-auth-failure".to_string()),
                    timestamp_utc: now_utc_rfc3339(),
                },
                error,
            );
        }
    };

    let request = match build_reporting_read_request(&query, &context) {
        Ok(request) => request,
        Err(error) => {
            return error_envelope_response(
                status_for_reason_code(error.code),
                dataset.as_str().to_string(),
                dataset.contract_key().to_string(),
                "unknown".to_string(),
                dataset.endpoint_path().to_string(),
                context,
                error,
            );
        }
    };

    let descriptor = match state
        .contract_port
        .resolve_contract_version(
            dataset.contract_key().to_string(),
            trim_to_option(query.contract_version.as_deref()),
            request.end_exclusive_utc.clone(),
        )
        .await
    {
        Ok(descriptor) => descriptor,
        Err(error) => {
            return error_envelope_response(
                status_for_reason_code(error.code),
                dataset.as_str().to_string(),
                dataset.contract_key().to_string(),
                "unknown".to_string(),
                dataset.endpoint_path().to_string(),
                context,
                error,
            );
        }
    };

    let result = match query_dataset(&state, dataset, request).await {
        Ok(result) => result,
        Err(error) => {
            return error_envelope_response(
                status_for_reason_code(error.code),
                dataset.as_str().to_string(),
                descriptor.contract_key,
                descriptor.contract_version,
                dataset.endpoint_path().to_string(),
                context,
                error,
            );
        }
    };

    let envelope = ReportingApiEnvelope {
        data: json!({
            "rows": result.rows,
        }),
        meta: ReportingApiMeta {
            dataset: dataset.as_str().to_string(),
            contract_key: descriptor.contract_key.clone(),
            contract_version: descriptor.contract_version.clone(),
            as_of_utc: result.as_of_utc.clone(),
            source: result.source.clone(),
            reason_code: result.reason_code.clone(),
            correlation_id: result.correlation_id.clone(),
            endpoint: dataset.endpoint_path().to_string(),
            actor_id: context.actor_id.clone(),
            actor_role: context.actor_role.clone(),
            timestamp_utc: now_utc_rfc3339(),
            start_inclusive_utc: Some(result.start_inclusive_utc.clone()),
            end_exclusive_utc: Some(result.end_exclusive_utc.clone()),
        },
        error: Value::Null,
    };
    emit_telemetry(
        &context,
        dataset.endpoint_path(),
        dataset.as_str(),
        &descriptor.contract_version,
        &result.reason_code,
    );
    (StatusCode::OK, Json(envelope)).into_response()
}

async fn query_dataset(
    state: &ReportingApiState,
    dataset: ReportingDatasetRoute,
    request: ReportingReadRequest,
) -> Result<DatasetQueryResult, ReportingReadModelError> {
    match dataset {
        ReportingDatasetRoute::Trades => {
            let response = state.query_port.query_trade_rows(request).await?;
            dataset_result_from_response(response)
        }
        ReportingDatasetRoute::Positions => {
            let response = state.query_port.query_position_rows(request).await?;
            dataset_result_from_response(response)
        }
        ReportingDatasetRoute::RiskEvents => {
            let response = state.query_port.query_risk_event_rows(request).await?;
            dataset_result_from_response(response)
        }
        ReportingDatasetRoute::Performance | ReportingDatasetRoute::AlphaAttribution => {
            let response = state.query_port.query_performance_rows(request).await?;
            dataset_result_from_response(response)
        }
    }
}

fn dataset_result_from_response<T: Serialize>(
    response: ReportingDatasetResponse<T>,
) -> Result<DatasetQueryResult, ReportingReadModelError> {
    let rows = serde_json::to_value(response.rows).map_err(|error| {
        ReportingReadModelError::dependency_unavailable(format!(
            "unable to serialize dataset rows into contract envelope: {error}"
        ))
    })?;
    Ok(DatasetQueryResult {
        rows,
        as_of_utc: response.as_of_utc,
        source: response.source,
        reason_code: response.reason_code,
        correlation_id: response.correlation_id,
        start_inclusive_utc: response.start_inclusive_utc,
        end_exclusive_utc: response.end_exclusive_utc,
    })
}

fn build_reporting_read_request(
    query: &ReportingDatasetQuery,
    context: &RequestContext,
) -> Result<ReportingReadRequest, ReportingReadModelError> {
    let start_inclusive_utc =
        required_query_field("start_inclusive_utc", query.start_inclusive_utc.as_deref())?;
    let end_exclusive_utc =
        required_query_field("end_exclusive_utc", query.end_exclusive_utc.as_deref())?;
    let limit = match trim_to_option(query.limit.as_deref()) {
        Some(raw) => raw.parse::<i64>().map_err(|_| {
            invalid_payload_error_with_field("limit must be a signed integer", "limit")
        })?,
        None => DEFAULT_REPORTING_LIMIT,
    };

    Ok(ReportingReadRequest {
        actor_role: context.actor_role.clone(),
        start_inclusive_utc,
        end_exclusive_utc,
        market_id: trim_to_option(query.market_id.as_deref()),
        alpha_id: trim_to_option(query.alpha_id.as_deref()),
        correlation_id: Some(context.correlation_id.clone()),
        limit,
    })
}

fn extract_request_context(
    headers: &HeaderMap,
    query_correlation_id: Option<&str>,
    correlation_dataset_scope: &str,
) -> Result<RequestContext, ReportingReadModelError> {
    let actor_role =
        header_value(headers, "x-actor-role").ok_or_else(|| ReportingReadModelError {
            code: "reporting_unauthorized",
            message: "missing required `x-actor-role` header".to_string(),
            field_errors: vec![ReportingValidationIssue {
                field: "x-actor-role",
                code: "reporting_unauthorized",
                message: "missing required `x-actor-role` header".to_string(),
            }],
        })?;
    authorize_reporting_read(&actor_role).map_err(map_contract_error)?;
    let actor_id =
        header_value(headers, "x-actor-id").unwrap_or_else(|| "unknown-actor".to_string());
    let correlation_id = trim_to_option(query_correlation_id)
        .or_else(|| header_value(headers, "x-correlation-id"))
        .unwrap_or_else(|| {
            format!(
                "corr-{correlation_dataset_scope}-{}",
                OffsetDateTime::now_utc().unix_timestamp_nanos()
            )
        });
    Ok(RequestContext {
        actor_id,
        actor_role,
        correlation_id,
        timestamp_utc: now_utc_rfc3339(),
    })
}

fn required_query_field(
    field: &'static str,
    value: Option<&str>,
) -> Result<String, ReportingReadModelError> {
    trim_to_option(value).ok_or_else(|| {
        invalid_payload_error_with_field(format!("{field} query parameter is required"), field)
    })
}

fn trim_to_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn status_for_reason_code(code: &str) -> StatusCode {
    match code {
        "reporting_invalid_payload" => StatusCode::BAD_REQUEST,
        "reporting_unauthorized" => StatusCode::FORBIDDEN,
        "reporting_dependency_unavailable"
        | "reporting_stale_dependency"
        | "reporting_persistence_unavailable"
        | "reporting_evidence_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_envelope_response(
    status: StatusCode,
    dataset: String,
    contract_key: String,
    contract_version: String,
    endpoint: String,
    context: RequestContext,
    error: ReportingReadModelError,
) -> Response {
    let reason_code = error.code.to_string();
    let envelope = ReportingApiEnvelope {
        data: Value::Null,
        meta: ReportingApiMeta {
            dataset: dataset.clone(),
            contract_key,
            contract_version: contract_version.clone(),
            as_of_utc: now_utc_rfc3339(),
            source: "reporting_service.contracts.v1".to_string(),
            reason_code: reason_code.clone(),
            correlation_id: context.correlation_id.clone(),
            endpoint: endpoint.clone(),
            actor_id: context.actor_id.clone(),
            actor_role: context.actor_role.clone(),
            timestamp_utc: context.timestamp_utc.clone(),
            start_inclusive_utc: None,
            end_exclusive_utc: None,
        },
        error: json!({
            "code": error.code,
            "message": error.message,
            "field_errors": error.field_errors,
        }),
    };
    emit_telemetry(
        &context,
        &endpoint,
        &dataset,
        &contract_version,
        &reason_code,
    );
    (status, Json(envelope)).into_response()
}

fn emit_telemetry(
    context: &RequestContext,
    endpoint: &str,
    dataset: &str,
    contract_version: &str,
    reason_code: &str,
) {
    let event = ReportingContractTelemetryEvent {
        event_name: "reporting_contract_query",
        actor_id: context.actor_id.clone(),
        actor_role: context.actor_role.clone(),
        endpoint: endpoint.to_string(),
        dataset: dataset.to_string(),
        contract_version: contract_version.to_string(),
        reason_code: reason_code.to_string(),
        correlation_id: context.correlation_id.clone(),
        timestamp_utc: now_utc_rfc3339(),
    };
    match serde_json::to_string(&event) {
        Ok(payload) => println!("{payload}"),
        Err(error) => eprintln!("failed to serialize reporting telemetry event: {error}"),
    }
}

fn now_utc_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn header_value(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn invalid_payload_error_with_field(
    message: impl Into<String>,
    field: &'static str,
) -> ReportingReadModelError {
    let message = message.into();
    ReportingReadModelError {
        code: "reporting_invalid_payload",
        message: message.clone(),
        field_errors: vec![ReportingValidationIssue {
            field,
            code: "reporting_invalid_payload",
            message,
        }],
    }
}

fn map_contract_error(error: ReportingContractError) -> ReportingReadModelError {
    ReportingReadModelError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn map_persistence_error(error: ApiContractVersionPersistenceError) -> ReportingReadModelError {
    ReportingReadModelError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

async fn load_replacement_release(
    pool: &PgPool,
    contract_key: &str,
    replacement_contract_version: Option<&str>,
) -> Result<Option<String>, ReportingReadModelError> {
    let Some(replacement_version) = replacement_contract_version else {
        return Ok(None);
    };
    let replacement = load_api_contract_version(pool, contract_key, replacement_version)
        .await
        .map_err(map_persistence_error)?
        .ok_or_else(|| {
            ReportingReadModelError::stale_dependency(
                "replacement contract metadata is missing for lifecycle validation",
            )
        })?;
    Ok(Some(replacement.release_at_utc))
}

fn build_descriptor(
    record: ApiContractVersionRecord,
    replacement_release_at: Option<&str>,
) -> Result<ReportingContractVersionDescriptor, ReportingReadModelError> {
    let descriptor =
        ReportingContractVersionDescriptor::from_record(record).map_err(map_contract_error)?;
    validate_contract_lifecycle(&descriptor, replacement_release_at).map_err(|error| {
        ReportingReadModelError::stale_dependency(format!(
            "contract lifecycle metadata is stale or invalid: {}",
            error.message
        ))
    })?;
    validate_artifacts_match_descriptor(&descriptor)?;
    Ok(descriptor)
}

fn validate_artifacts_match_descriptor(
    descriptor: &ReportingContractVersionDescriptor,
) -> Result<(), ReportingReadModelError> {
    let Some(schema_checksum) =
        expected_schema_checksum(&descriptor.contract_key, &descriptor.contract_version)
    else {
        return Err(ReportingReadModelError::dependency_unavailable(
            "schema artifact metadata is unavailable for requested contract version",
        ));
    };
    let Some(changelog_checksum) = expected_changelog_checksum(&descriptor.contract_version) else {
        return Err(ReportingReadModelError::dependency_unavailable(
            "changelog artifact metadata is unavailable for requested contract version",
        ));
    };
    let Some(schema_artifact) =
        schema_artifact_for(&descriptor.contract_key, &descriptor.contract_version)
    else {
        return Err(ReportingReadModelError::dependency_unavailable(
            "schema artifact payload is unavailable for requested contract version",
        ));
    };
    let Some(changelog_artifact) = changelog_artifact_for(&descriptor.contract_version) else {
        return Err(ReportingReadModelError::dependency_unavailable(
            "changelog artifact payload is unavailable for requested contract version",
        ));
    };

    if schema_artifact.artifact_path != descriptor.schema_artifact_path
        || schema_artifact.checksum_sha256 != schema_checksum
        || descriptor.schema_checksum_sha256 != schema_checksum
    {
        return Err(ReportingReadModelError::stale_dependency(
            "schema artifact path/checksum does not match active contract metadata",
        ));
    }
    if changelog_artifact.artifact_path != descriptor.changelog_artifact_path
        || changelog_artifact.checksum_sha256 != changelog_checksum
        || descriptor.changelog_checksum_sha256 != changelog_checksum
    {
        return Err(ReportingReadModelError::stale_dependency(
            "changelog artifact path/checksum does not match active contract metadata",
        ));
    }

    let expected_record_checksum = compute_record_checksum(
        &descriptor.contract_key,
        &descriptor.contract_version,
        &descriptor.schema_checksum_sha256,
        &descriptor.changelog_checksum_sha256,
    );
    if expected_record_checksum != descriptor.record_checksum_sha256 {
        return Err(ReportingReadModelError::stale_dependency(
            "contract metadata record checksum does not match artifact checksums",
        ));
    }
    Ok(())
}

fn ensure_contract_support_window(
    descriptor: &ReportingContractVersionDescriptor,
    as_of_utc: &str,
) -> Result<(), ReportingReadModelError> {
    if descriptor.lifecycle_status == ContractLifecycleStatus::Active {
        return Ok(());
    }
    let as_of = parse_utc_timestamp("as_of_utc", as_of_utc).map_err(map_contract_error)?;
    let support_until = descriptor
        .lifecycle_window
        .backward_compatible_until_utc
        .as_deref()
        .ok_or_else(|| {
            ReportingReadModelError::stale_dependency(
                "non-active contract metadata is missing backward_compatible_until_utc",
            )
        })?;
    let support_until = parse_utc_timestamp("backward_compatible_until_utc", support_until)
        .map_err(map_contract_error)?;
    if as_of >= support_until {
        return Err(ReportingReadModelError::stale_dependency(
            "contract version support window has elapsed",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct ContractVersionMetadata {
    contract_key: String,
    contract_version: String,
    lifecycle_status: String,
    is_active: bool,
    release_at_utc: String,
    deprecation_notice_at_utc: Option<String>,
    backward_compatible_until_utc: Option<String>,
    sunset_at_utc: Option<String>,
    replacement_contract_version: Option<String>,
    schema_artifact_path: String,
    changelog_artifact_path: String,
    schema_checksum_sha256: String,
    changelog_checksum_sha256: String,
    record_checksum_sha256: String,
}

impl ContractVersionMetadata {
    fn from_descriptor(descriptor: ReportingContractVersionDescriptor) -> Self {
        Self {
            contract_key: descriptor.contract_key,
            contract_version: descriptor.contract_version,
            lifecycle_status: descriptor.lifecycle_status.as_str().to_string(),
            is_active: descriptor.is_active,
            release_at_utc: descriptor.lifecycle_window.release_at_utc,
            deprecation_notice_at_utc: descriptor.lifecycle_window.deprecation_notice_at_utc,
            backward_compatible_until_utc: descriptor
                .lifecycle_window
                .backward_compatible_until_utc,
            sunset_at_utc: descriptor.lifecycle_window.sunset_at_utc,
            replacement_contract_version: descriptor.lifecycle_window.replacement_contract_version,
            schema_artifact_path: descriptor.schema_artifact_path,
            changelog_artifact_path: descriptor.changelog_artifact_path,
            schema_checksum_sha256: descriptor.schema_checksum_sha256,
            changelog_checksum_sha256: descriptor.changelog_checksum_sha256,
            record_checksum_sha256: descriptor.record_checksum_sha256,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct StaticReportingQueryPort {
        trade_result: Result<ReportingDatasetResponse<TradeReportingRow>, ReportingReadModelError>,
        position_result:
            Result<ReportingDatasetResponse<PositionReportingRow>, ReportingReadModelError>,
        risk_result:
            Result<ReportingDatasetResponse<RiskEventReportingRow>, ReportingReadModelError>,
        performance_result:
            Result<ReportingDatasetResponse<PerformanceReportingRow>, ReportingReadModelError>,
    }

    impl ReportingQueryPort for StaticReportingQueryPort {
        fn query_trade_rows(
            &self,
            _request: ReportingReadRequest,
        ) -> AsyncResult<Result<ReportingDatasetResponse<TradeReportingRow>, ReportingReadModelError>>
        {
            let result = self.trade_result.clone();
            Box::pin(async move { result })
        }

        fn query_position_rows(
            &self,
            _request: ReportingReadRequest,
        ) -> AsyncResult<
            Result<ReportingDatasetResponse<PositionReportingRow>, ReportingReadModelError>,
        > {
            let result = self.position_result.clone();
            Box::pin(async move { result })
        }

        fn query_risk_event_rows(
            &self,
            _request: ReportingReadRequest,
        ) -> AsyncResult<
            Result<ReportingDatasetResponse<RiskEventReportingRow>, ReportingReadModelError>,
        > {
            let result = self.risk_result.clone();
            Box::pin(async move { result })
        }

        fn query_performance_rows(
            &self,
            _request: ReportingReadRequest,
        ) -> AsyncResult<
            Result<ReportingDatasetResponse<PerformanceReportingRow>, ReportingReadModelError>,
        > {
            let result = self.performance_result.clone();
            Box::pin(async move { result })
        }
    }

    #[derive(Clone)]
    struct StaticContractVersionPort {
        resolve_result: Result<ReportingContractVersionDescriptor, ReportingReadModelError>,
        list_result: Result<Vec<ReportingContractVersionDescriptor>, ReportingReadModelError>,
    }

    impl ContractVersionPort for StaticContractVersionPort {
        fn resolve_contract_version(
            &self,
            _contract_key: String,
            _requested_version: Option<String>,
            _as_of_utc: String,
        ) -> AsyncResult<Result<ReportingContractVersionDescriptor, ReportingReadModelError>>
        {
            let result = self.resolve_result.clone();
            Box::pin(async move { result })
        }

        fn list_contract_versions(
            &self,
            _contract_key: String,
        ) -> AsyncResult<Result<Vec<ReportingContractVersionDescriptor>, ReportingReadModelError>>
        {
            let result = self.list_result.clone();
            Box::pin(async move { result })
        }
    }

    #[derive(Clone)]
    struct TrackingContractVersionPort {
        descriptor: ReportingContractVersionDescriptor,
        observed_requested_versions: Arc<std::sync::Mutex<Vec<Option<String>>>>,
    }

    impl ContractVersionPort for TrackingContractVersionPort {
        fn resolve_contract_version(
            &self,
            _contract_key: String,
            requested_version: Option<String>,
            _as_of_utc: String,
        ) -> AsyncResult<Result<ReportingContractVersionDescriptor, ReportingReadModelError>>
        {
            self.observed_requested_versions
                .lock()
                .expect("requested version lock should be available")
                .push(requested_version);
            let descriptor = self.descriptor.clone();
            Box::pin(async move { Ok(descriptor) })
        }

        fn list_contract_versions(
            &self,
            _contract_key: String,
        ) -> AsyncResult<Result<Vec<ReportingContractVersionDescriptor>, ReportingReadModelError>>
        {
            let descriptor = self.descriptor.clone();
            Box::pin(async move { Ok(vec![descriptor]) })
        }
    }

    fn sample_descriptor(contract_key: &str) -> ReportingContractVersionDescriptor {
        let schema_checksum =
            expected_schema_checksum(contract_key, artifacts::CONTRACT_VERSION_V1)
                .expect("schema checksum must exist");
        let changelog_checksum = expected_changelog_checksum(artifacts::CONTRACT_VERSION_V1)
            .expect("changelog checksum must exist");
        let schema_artifact = schema_artifact_for(contract_key, artifacts::CONTRACT_VERSION_V1)
            .expect("schema artifact must exist");
        let changelog_artifact = changelog_artifact_for(artifacts::CONTRACT_VERSION_V1)
            .expect("changelog artifact must exist");
        ReportingContractVersionDescriptor {
            contract_key: contract_key.to_string(),
            contract_version: artifacts::CONTRACT_VERSION_V1.to_string(),
            lifecycle_status: ContractLifecycleStatus::Active,
            is_active: true,
            lifecycle_window: crate::contracts::lifecycle::ContractLifecycleWindow {
                release_at_utc: "2026-04-07T03:30:11Z".to_string(),
                deprecation_notice_at_utc: None,
                backward_compatible_until_utc: None,
                sunset_at_utc: None,
                replacement_contract_version: None,
            },
            schema_artifact_path: schema_artifact.artifact_path,
            changelog_artifact_path: changelog_artifact.artifact_path,
            schema_checksum_sha256: schema_checksum.clone(),
            changelog_checksum_sha256: changelog_checksum.clone(),
            record_checksum_sha256: compute_record_checksum(
                contract_key,
                artifacts::CONTRACT_VERSION_V1,
                &schema_checksum,
                &changelog_checksum,
            ),
        }
    }

    fn sample_trade_response(row_count: usize) -> ReportingDatasetResponse<TradeReportingRow> {
        let mut rows = Vec::new();
        if row_count > 0 {
            rows.push(TradeReportingRow {
                report_row_id: "trade-row-001".to_string(),
                order_id: "order-001".to_string(),
                trade_id: "trade-001".to_string(),
                market_id: "market-btc-election".to_string(),
                asset_id: "asset-btc".to_string(),
                lifecycle_state: "filled".to_string(),
                event_status: "matched".to_string(),
                occurred_at_utc: "2026-04-07T03:30:11Z".to_string(),
                evidence: domain::reporting::ReportingEvidenceMetadata {
                    as_of_utc: "2026-04-07T03:30:11Z".to_string(),
                    source: "reporting.read-models.v1".to_string(),
                    reason_code: "reporting_ready".to_string(),
                    correlation_id: "corr-reporting-api-tests".to_string(),
                    run_id: Some("run-001".to_string()),
                    snapshot_id: Some("snapshot-001".to_string()),
                },
            });
        }
        ReportingDatasetResponse {
            dataset: crate::read_models::queries::ReportingReadDataset::Trade,
            data_state: if row_count == 0 { "empty" } else { "ready" }.to_string(),
            as_of_utc: "2026-04-07T03:30:11Z".to_string(),
            source: "reporting_service.read_models.v1".to_string(),
            reason_code: if row_count == 0 {
                "reporting_empty_window".to_string()
            } else {
                "reporting_ready".to_string()
            },
            correlation_id: "corr-reporting-api-tests".to_string(),
            start_inclusive_utc: "2026-04-07T02:00:00Z".to_string(),
            end_exclusive_utc: "2026-04-07T03:00:00Z".to_string(),
            rows,
            audit_event: crate::read_models::queries::ReportingReadAuditEvent {
                dataset: "trade".to_string(),
                row_count,
                as_of_utc: "2026-04-07T03:30:11Z".to_string(),
                source: "reporting_service.read_models.v1".to_string(),
                reason_code: if row_count == 0 {
                    "reporting_empty_window".to_string()
                } else {
                    "reporting_ready".to_string()
                },
                correlation_id: "corr-reporting-api-tests".to_string(),
            },
        }
    }

    fn sample_position_response() -> ReportingDatasetResponse<PositionReportingRow> {
        ReportingDatasetResponse {
            dataset: crate::read_models::queries::ReportingReadDataset::Position,
            data_state: "empty".to_string(),
            as_of_utc: "2026-04-07T03:30:11Z".to_string(),
            source: "reporting_service.read_models.v1".to_string(),
            reason_code: "reporting_empty_window".to_string(),
            correlation_id: "corr-reporting-api-tests".to_string(),
            start_inclusive_utc: "2026-04-07T02:00:00Z".to_string(),
            end_exclusive_utc: "2026-04-07T03:00:00Z".to_string(),
            rows: Vec::new(),
            audit_event: crate::read_models::queries::ReportingReadAuditEvent {
                dataset: "position".to_string(),
                row_count: 0,
                as_of_utc: "2026-04-07T03:30:11Z".to_string(),
                source: "reporting_service.read_models.v1".to_string(),
                reason_code: "reporting_empty_window".to_string(),
                correlation_id: "corr-reporting-api-tests".to_string(),
            },
        }
    }

    fn sample_risk_response() -> ReportingDatasetResponse<RiskEventReportingRow> {
        ReportingDatasetResponse {
            dataset: crate::read_models::queries::ReportingReadDataset::RiskEvent,
            data_state: "empty".to_string(),
            as_of_utc: "2026-04-07T03:30:11Z".to_string(),
            source: "reporting_service.read_models.v1".to_string(),
            reason_code: "reporting_empty_window".to_string(),
            correlation_id: "corr-reporting-api-tests".to_string(),
            start_inclusive_utc: "2026-04-07T02:00:00Z".to_string(),
            end_exclusive_utc: "2026-04-07T03:00:00Z".to_string(),
            rows: Vec::new(),
            audit_event: crate::read_models::queries::ReportingReadAuditEvent {
                dataset: "risk_event".to_string(),
                row_count: 0,
                as_of_utc: "2026-04-07T03:30:11Z".to_string(),
                source: "reporting_service.read_models.v1".to_string(),
                reason_code: "reporting_empty_window".to_string(),
                correlation_id: "corr-reporting-api-tests".to_string(),
            },
        }
    }

    fn sample_performance_response() -> ReportingDatasetResponse<PerformanceReportingRow> {
        ReportingDatasetResponse {
            dataset: crate::read_models::queries::ReportingReadDataset::Performance,
            data_state: "empty".to_string(),
            as_of_utc: "2026-04-07T03:30:11Z".to_string(),
            source: "reporting_service.read_models.v1".to_string(),
            reason_code: "reporting_empty_window".to_string(),
            correlation_id: "corr-reporting-api-tests".to_string(),
            start_inclusive_utc: "2026-04-07T02:00:00Z".to_string(),
            end_exclusive_utc: "2026-04-07T03:00:00Z".to_string(),
            rows: Vec::new(),
            audit_event: crate::read_models::queries::ReportingReadAuditEvent {
                dataset: "performance".to_string(),
                row_count: 0,
                as_of_utc: "2026-04-07T03:30:11Z".to_string(),
                source: "reporting_service.read_models.v1".to_string(),
                reason_code: "reporting_empty_window".to_string(),
                correlation_id: "corr-reporting-api-tests".to_string(),
            },
        }
    }

    fn success_state(row_count: usize) -> ReportingApiState {
        ReportingApiState::new(
            Arc::new(StaticReportingQueryPort {
                trade_result: Ok(sample_trade_response(row_count)),
                position_result: Ok(sample_position_response()),
                risk_result: Ok(sample_risk_response()),
                performance_result: Ok(sample_performance_response()),
            }),
            Arc::new(StaticContractVersionPort {
                resolve_result: Ok(sample_descriptor(artifacts::CONTRACT_KEY_TRADES)),
                list_result: Ok(vec![sample_descriptor(artifacts::CONTRACT_KEY_TRADES)]),
            }),
        )
    }

    fn request(uri: &str, role: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header("x-actor-id", "integration-actor")
            .header("x-actor-role", role)
            .header("x-correlation-id", "corr-reporting-api-tests")
            .body(Body::empty())
            .expect("request should build")
    }

    async fn json_body(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body bytes should decode");
        serde_json::from_slice(&body).expect("response body should be valid JSON")
    }

    #[tokio::test]
    async fn versioned_trades_route_returns_success_envelope_with_contract_metadata() {
        let app = reporting_router(success_state(1));
        let response = app
            .oneshot(request(
                "/api/v1/reporting/trades?start_inclusive_utc=2026-04-07T02:00:00Z&end_exclusive_utc=2026-04-07T03:00:00Z",
                "read_only_analytics",
            ))
            .await
            .expect("route should respond");
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["meta"]["contract_version"], "v1");
        assert_eq!(body["meta"]["reason_code"], "reporting_ready");
        assert_eq!(body["meta"]["dataset"], "trades");
        assert_eq!(body["data"]["rows"].as_array().map(Vec::len), Some(1));
        assert!(body["error"].is_null());
    }

    #[tokio::test]
    async fn versioned_trades_route_returns_empty_dataset_state_deterministically() {
        let app = reporting_router(success_state(0));
        let response = app
            .oneshot(request(
                "/api/v1/reporting/trades?start_inclusive_utc=2026-04-07T02:00:00Z&end_exclusive_utc=2026-04-07T03:00:00Z",
                "read_only_analytics",
            ))
            .await
            .expect("route should respond");
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["meta"]["reason_code"], "reporting_empty_window");
        assert_eq!(body["data"]["rows"].as_array().map(Vec::len), Some(0));
    }

    #[tokio::test]
    async fn versioned_route_rejects_missing_required_query_windows() {
        let app = reporting_router(success_state(1));
        let response = app
            .oneshot(request(
                "/api/v1/reporting/trades?start_inclusive_utc=2026-04-07T02:00:00Z",
                "read_only_analytics",
            ))
            .await
            .expect("route should respond");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = json_body(response).await;
        assert_eq!(body["error"]["code"], "reporting_invalid_payload");
        assert_eq!(
            body["error"]["field_errors"][0]["field"],
            "end_exclusive_utc"
        );
        assert!(body["data"].is_null());
    }

    #[tokio::test]
    async fn versioned_route_propagates_unauthorized_failures() {
        let unauthorized_error = ReportingReadModelError {
            code: "reporting_unauthorized",
            message: "read role is not authorized".to_string(),
            field_errors: Vec::new(),
        };
        let state = ReportingApiState::new(
            Arc::new(StaticReportingQueryPort {
                trade_result: Err(unauthorized_error),
                position_result: Ok(sample_position_response()),
                risk_result: Ok(sample_risk_response()),
                performance_result: Ok(sample_performance_response()),
            }),
            Arc::new(StaticContractVersionPort {
                resolve_result: Ok(sample_descriptor(artifacts::CONTRACT_KEY_TRADES)),
                list_result: Ok(vec![sample_descriptor(artifacts::CONTRACT_KEY_TRADES)]),
            }),
        );
        let app = reporting_router(state);
        let response = app
            .oneshot(request(
                "/api/v1/reporting/trades?start_inclusive_utc=2026-04-07T02:00:00Z&end_exclusive_utc=2026-04-07T03:00:00Z",
                "guest",
            ))
            .await
            .expect("route should respond");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = json_body(response).await;
        assert_eq!(body["error"]["code"], "reporting_unauthorized");
    }

    #[tokio::test]
    async fn versioned_route_passes_contract_version_override_to_resolution_port() {
        let observed_requested_versions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let state = ReportingApiState::new(
            Arc::new(StaticReportingQueryPort {
                trade_result: Ok(sample_trade_response(1)),
                position_result: Ok(sample_position_response()),
                risk_result: Ok(sample_risk_response()),
                performance_result: Ok(sample_performance_response()),
            }),
            Arc::new(TrackingContractVersionPort {
                descriptor: sample_descriptor(artifacts::CONTRACT_KEY_TRADES),
                observed_requested_versions: observed_requested_versions.clone(),
            }),
        );
        let app = reporting_router(state);

        let explicit_response = app
            .clone()
            .oneshot(request(
                "/api/v1/reporting/trades?start_inclusive_utc=2026-04-07T02:00:00Z&end_exclusive_utc=2026-04-07T03:00:00Z&contract_version=%20v1%20",
                "read_only_analytics",
            ))
            .await
            .expect("route should respond");
        assert_eq!(explicit_response.status(), StatusCode::OK);

        let fallback_response = app
            .oneshot(request(
                "/api/v1/reporting/trades?start_inclusive_utc=2026-04-07T02:00:00Z&end_exclusive_utc=2026-04-07T03:00:00Z",
                "read_only_analytics",
            ))
            .await
            .expect("route should respond");
        assert_eq!(fallback_response.status(), StatusCode::OK);

        let observed = observed_requested_versions
            .lock()
            .expect("requested version lock should be available")
            .clone();
        assert_eq!(observed, vec![Some("v1".to_string()), None]);
    }

    #[tokio::test]
    async fn contract_discovery_routes_reject_unauthorized_roles() {
        let app = reporting_router(success_state(1));
        let response = app
            .oneshot(request(
                "/api/v1/reporting/contracts/trades/versions",
                "guest",
            ))
            .await
            .expect("route should respond");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = json_body(response).await;
        assert_eq!(body["error"]["code"], "reporting_unauthorized");
        assert!(body["data"].is_null());
    }

    #[tokio::test]
    async fn contract_artifact_routes_reject_unauthorized_roles() {
        let app = reporting_router(success_state(1));
        let response = app
            .oneshot(request(
                "/api/v1/reporting/contracts/trades/versions/v1/schema",
                "guest",
            ))
            .await
            .expect("route should respond");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = json_body(response).await;
        assert_eq!(body["error"]["code"], "reporting_unauthorized");
        assert!(body["data"].is_null());
    }

    #[tokio::test]
    async fn versioned_route_fails_closed_on_dependency_unavailable() {
        let dependency_error = ReportingReadModelError {
            code: "reporting_dependency_unavailable",
            message: "normalized projections unavailable".to_string(),
            field_errors: Vec::new(),
        };
        let state = ReportingApiState::new(
            Arc::new(StaticReportingQueryPort {
                trade_result: Err(dependency_error),
                position_result: Ok(sample_position_response()),
                risk_result: Ok(sample_risk_response()),
                performance_result: Ok(sample_performance_response()),
            }),
            Arc::new(StaticContractVersionPort {
                resolve_result: Ok(sample_descriptor(artifacts::CONTRACT_KEY_TRADES)),
                list_result: Ok(vec![sample_descriptor(artifacts::CONTRACT_KEY_TRADES)]),
            }),
        );
        let app = reporting_router(state);
        let response = app
            .oneshot(request(
                "/api/v1/reporting/trades?start_inclusive_utc=2026-04-07T02:00:00Z&end_exclusive_utc=2026-04-07T03:00:00Z",
                "read_only_analytics",
            ))
            .await
            .expect("route should respond");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = json_body(response).await;
        assert_eq!(body["error"]["code"], "reporting_dependency_unavailable");
    }

    #[tokio::test]
    async fn versioned_route_fails_closed_on_stale_contract_metadata() {
        let stale_error = ReportingReadModelError {
            code: "reporting_stale_dependency",
            message: "contract metadata checksum mismatch".to_string(),
            field_errors: Vec::new(),
        };
        let state = ReportingApiState::new(
            Arc::new(StaticReportingQueryPort {
                trade_result: Ok(sample_trade_response(1)),
                position_result: Ok(sample_position_response()),
                risk_result: Ok(sample_risk_response()),
                performance_result: Ok(sample_performance_response()),
            }),
            Arc::new(StaticContractVersionPort {
                resolve_result: Err(stale_error),
                list_result: Ok(vec![sample_descriptor(artifacts::CONTRACT_KEY_TRADES)]),
            }),
        );
        let app = reporting_router(state);
        let response = app
            .oneshot(request(
                "/api/v1/reporting/trades?start_inclusive_utc=2026-04-07T02:00:00Z&end_exclusive_utc=2026-04-07T03:00:00Z",
                "read_only_analytics",
            ))
            .await
            .expect("route should respond");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = json_body(response).await;
        assert_eq!(body["error"]["code"], "reporting_stale_dependency");
    }

    #[tokio::test]
    async fn schema_and_changelog_routes_publish_discoverable_contract_artifacts() {
        let state = ReportingApiState::new(
            Arc::new(StaticReportingQueryPort {
                trade_result: Ok(sample_trade_response(1)),
                position_result: Ok(sample_position_response()),
                risk_result: Ok(sample_risk_response()),
                performance_result: Ok(sample_performance_response()),
            }),
            Arc::new(StaticContractVersionPort {
                resolve_result: Ok(sample_descriptor(artifacts::CONTRACT_KEY_TRADES)),
                list_result: Ok(vec![sample_descriptor(artifacts::CONTRACT_KEY_TRADES)]),
            }),
        );
        let app = reporting_router(state.clone());
        let schema_response = app
            .clone()
            .oneshot(request(
                "/api/v1/reporting/contracts/trades/versions/v1/schema",
                "read_only_analytics",
            ))
            .await
            .expect("schema route should respond");
        assert_eq!(schema_response.status(), StatusCode::OK);
        let schema_body = json_body(schema_response).await;
        assert_eq!(schema_body["data"]["artifact_kind"], "schema");
        assert_eq!(
            schema_body["data"]["contract_version"],
            artifacts::CONTRACT_VERSION_V1
        );

        let changelog_response = app
            .oneshot(request(
                "/api/v1/reporting/contracts/trades/versions/v1/changelog",
                "read_only_analytics",
            ))
            .await
            .expect("changelog route should respond");
        assert_eq!(changelog_response.status(), StatusCode::OK);
        let changelog_body = json_body(changelog_response).await;
        assert_eq!(changelog_body["data"]["artifact_kind"], "changelog");
        assert_eq!(
            changelog_body["data"]["payload"]["contract_version"],
            artifacts::CONTRACT_VERSION_V1
        );
    }

    #[tokio::test]
    async fn schema_payload_stays_in_sync_with_runtime_envelope_shape() {
        let app = reporting_router(success_state(1));
        let response = app
            .oneshot(request(
                "/api/v1/reporting/trades?start_inclusive_utc=2026-04-07T02:00:00Z&end_exclusive_utc=2026-04-07T03:00:00Z",
                "read_only_analytics",
            ))
            .await
            .expect("route should respond");
        let runtime_payload = json_body(response).await;

        let schema = schema_artifact_for(
            artifacts::CONTRACT_KEY_TRADES,
            artifacts::CONTRACT_VERSION_V1,
        )
        .expect("schema artifact must exist");
        let required = schema
            .payload
            .get("required")
            .and_then(Value::as_array)
            .expect("schema required array must exist")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        for key in required {
            assert!(
                runtime_payload.get(key).is_some(),
                "runtime payload should include `{key}`"
            );
        }
    }
}
