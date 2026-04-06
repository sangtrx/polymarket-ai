use domain::reporting::{
    DEFAULT_REPORTING_LIMIT, PerformanceReportingRow, ReportingContractError,
    ReportingEvidenceMetadata, ReportingReadQuery, ReportingReasonCode, ReportingValidationIssue,
    RiskEventReportingRow, TradeReportingRow, authorize_reporting_read, build_reporting_read_query,
    parse_utc_timestamp, sort_performance_reporting_rows, sort_position_reporting_rows,
    sort_risk_event_reporting_rows, sort_trade_reporting_rows,
};
use persistence::postgres::reporting_read_models::{
    ReportingReadModelPersistenceError, load_performance_reporting_rows,
    load_position_reporting_rows, load_risk_event_reporting_rows, load_trade_reporting_rows,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};

use domain::reporting::PositionReportingRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportingReadDataset {
    Trade,
    Position,
    RiskEvent,
    Performance,
}

impl ReportingReadDataset {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trade => "trade",
            Self::Position => "position",
            Self::RiskEvent => "risk_event",
            Self::Performance => "performance",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportingDependencyStatus {
    pub projections_available: bool,
    pub projections_fresh: bool,
}

impl Default for ReportingDependencyStatus {
    fn default() -> Self {
        Self {
            projections_available: true,
            projections_fresh: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportingReadRequest {
    pub actor_role: String,
    pub start_inclusive_utc: String,
    pub end_exclusive_utc: String,
    pub market_id: Option<String>,
    pub alpha_id: Option<String>,
    pub correlation_id: Option<String>,
    pub limit: i64,
}

impl ReportingReadRequest {
    pub fn with_default_limit(
        actor_role: impl Into<String>,
        start_inclusive_utc: impl Into<String>,
        end_exclusive_utc: impl Into<String>,
    ) -> Self {
        Self {
            actor_role: actor_role.into(),
            start_inclusive_utc: start_inclusive_utc.into(),
            end_exclusive_utc: end_exclusive_utc.into(),
            market_id: None,
            alpha_id: None,
            correlation_id: None,
            limit: DEFAULT_REPORTING_LIMIT,
        }
    }

    fn to_query(&self) -> Result<ReportingReadQuery, ReportingReadModelError> {
        build_reporting_read_query(
            &self.start_inclusive_utc,
            &self.end_exclusive_utc,
            self.market_id.as_deref(),
            self.alpha_id.as_deref(),
            self.correlation_id.as_deref(),
            self.limit,
        )
        .map_err(ReportingReadModelError::from_contract_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportingReadAuditEvent {
    pub dataset: String,
    pub row_count: usize,
    pub as_of_utc: String,
    pub source: String,
    pub reason_code: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportingDatasetResponse<T> {
    pub dataset: ReportingReadDataset,
    pub data_state: String,
    pub as_of_utc: String,
    pub source: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub start_inclusive_utc: String,
    pub end_exclusive_utc: String,
    pub rows: Vec<T>,
    pub audit_event: ReportingReadAuditEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportingReadModelError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReportingValidationIssue>,
}

impl ReportingReadModelError {
    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn stale_dependency(message: impl Into<String>) -> Self {
        Self {
            code: ReportingReasonCode::StaleDependency.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn evidence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingReasonCode::EvidenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn from_contract_error(error: ReportingContractError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        }
    }

    fn from_persistence_error(error: ReportingReadModelPersistenceError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        }
    }
}

impl Display for ReportingReadModelError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ReportingReadModelError {}

#[derive(Debug, Clone)]
pub struct ReportingReadModelOrchestrator {
    pool: Option<PgPool>,
    dependency_status: ReportingDependencyStatus,
}

impl ReportingReadModelOrchestrator {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: Some(pool),
            dependency_status: ReportingDependencyStatus::default(),
        }
    }

    pub fn without_pool() -> Self {
        Self {
            pool: None,
            dependency_status: ReportingDependencyStatus::default(),
        }
    }

    pub fn with_dependency_status(mut self, dependency_status: ReportingDependencyStatus) -> Self {
        self.dependency_status = dependency_status;
        self
    }

    pub fn bootstrap_from_env() -> Result<Option<Self>, ReportingReadModelError> {
        let database_url = match env::var("DATABASE_URL") {
            Ok(value) => value,
            Err(env::VarError::NotPresent) => return Ok(None),
            Err(error) => {
                return Err(ReportingReadModelError::dependency_unavailable(format!(
                    "failed to read DATABASE_URL: {error}"
                )));
            }
        };
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect_lazy(&database_url)
            .map_err(|error| {
                ReportingReadModelError::dependency_unavailable(format!(
                    "failed to initialize reporting read-model pool: {error}"
                ))
            })?;
        Ok(Some(Self::new(pool)))
    }

    pub fn warmup_status(&self) -> &'static str {
        match (
            self.pool.is_some(),
            self.dependency_status.projections_available,
        ) {
            (true, true) => "read-model-adapter-initialized",
            (true, false) => "read-model-adapter-initialized-dependency-unavailable",
            (false, _) => "read-model-adapter-unavailable",
        }
    }

    pub async fn query_trade_rows(
        &self,
        request: &ReportingReadRequest,
    ) -> Result<ReportingDatasetResponse<TradeReportingRow>, ReportingReadModelError> {
        authorize_reporting_read(&request.actor_role)
            .map_err(ReportingReadModelError::from_contract_error)?;
        self.ensure_dependencies()?;
        let query = request.to_query()?;
        let pool = self.pool.as_ref().ok_or_else(|| {
            ReportingReadModelError::dependency_unavailable(
                "reporting read-model pool is unavailable",
            )
        })?;

        let mut rows = load_trade_reporting_rows(pool, &query)
            .await
            .map_err(ReportingReadModelError::from_persistence_error)?;
        sort_trade_reporting_rows(&mut rows);
        finalize_dataset_response(ReportingReadDataset::Trade, &query, rows)
    }

    pub async fn query_position_rows(
        &self,
        request: &ReportingReadRequest,
    ) -> Result<ReportingDatasetResponse<PositionReportingRow>, ReportingReadModelError> {
        authorize_reporting_read(&request.actor_role)
            .map_err(ReportingReadModelError::from_contract_error)?;
        self.ensure_dependencies()?;
        let query = request.to_query()?;
        let pool = self.pool.as_ref().ok_or_else(|| {
            ReportingReadModelError::dependency_unavailable(
                "reporting read-model pool is unavailable",
            )
        })?;

        let mut rows = load_position_reporting_rows(pool, &query)
            .await
            .map_err(ReportingReadModelError::from_persistence_error)?;
        sort_position_reporting_rows(&mut rows);
        finalize_dataset_response(ReportingReadDataset::Position, &query, rows)
    }

    pub async fn query_risk_event_rows(
        &self,
        request: &ReportingReadRequest,
    ) -> Result<ReportingDatasetResponse<RiskEventReportingRow>, ReportingReadModelError> {
        authorize_reporting_read(&request.actor_role)
            .map_err(ReportingReadModelError::from_contract_error)?;
        self.ensure_dependencies()?;
        let query = request.to_query()?;
        let pool = self.pool.as_ref().ok_or_else(|| {
            ReportingReadModelError::dependency_unavailable(
                "reporting read-model pool is unavailable",
            )
        })?;

        let mut rows = load_risk_event_reporting_rows(pool, &query)
            .await
            .map_err(ReportingReadModelError::from_persistence_error)?;
        sort_risk_event_reporting_rows(&mut rows);
        finalize_dataset_response(ReportingReadDataset::RiskEvent, &query, rows)
    }

    pub async fn query_performance_rows(
        &self,
        request: &ReportingReadRequest,
    ) -> Result<ReportingDatasetResponse<PerformanceReportingRow>, ReportingReadModelError> {
        authorize_reporting_read(&request.actor_role)
            .map_err(ReportingReadModelError::from_contract_error)?;
        self.ensure_dependencies()?;
        let query = request.to_query()?;
        let pool = self.pool.as_ref().ok_or_else(|| {
            ReportingReadModelError::dependency_unavailable(
                "reporting read-model pool is unavailable",
            )
        })?;

        let mut rows = load_performance_reporting_rows(pool, &query)
            .await
            .map_err(ReportingReadModelError::from_persistence_error)?;
        sort_performance_reporting_rows(&mut rows);
        finalize_dataset_response(ReportingReadDataset::Performance, &query, rows)
    }

    fn ensure_dependencies(&self) -> Result<(), ReportingReadModelError> {
        if !self.dependency_status.projections_available {
            return Err(ReportingReadModelError::dependency_unavailable(
                "normalized reporting projections are unavailable",
            ));
        }
        if !self.dependency_status.projections_fresh {
            return Err(ReportingReadModelError::stale_dependency(
                "normalized reporting projections are stale",
            ));
        }
        Ok(())
    }
}

trait HasEvidence {
    fn evidence(&self) -> &ReportingEvidenceMetadata;
}

impl HasEvidence for TradeReportingRow {
    fn evidence(&self) -> &ReportingEvidenceMetadata {
        &self.evidence
    }
}

impl HasEvidence for PositionReportingRow {
    fn evidence(&self) -> &ReportingEvidenceMetadata {
        &self.evidence
    }
}

impl HasEvidence for RiskEventReportingRow {
    fn evidence(&self) -> &ReportingEvidenceMetadata {
        &self.evidence
    }
}

impl HasEvidence for PerformanceReportingRow {
    fn evidence(&self) -> &ReportingEvidenceMetadata {
        &self.evidence
    }
}

fn finalize_dataset_response<T: HasEvidence>(
    dataset: ReportingReadDataset,
    query: &ReportingReadQuery,
    rows: Vec<T>,
) -> Result<ReportingDatasetResponse<T>, ReportingReadModelError> {
    for row in &rows {
        ensure_evidence_metadata(row.evidence())?;
    }

    let data_state = if rows.is_empty() { "empty" } else { "ready" }.to_string();
    let reason_code = if rows.is_empty() {
        ReportingReasonCode::EmptyWindow.code().to_string()
    } else {
        ReportingReasonCode::Ready.code().to_string()
    };
    let as_of_utc = rows
        .first()
        .map(|row| row.evidence().as_of_utc.clone())
        .unwrap_or_else(|| query.end_exclusive_utc.clone());
    let correlation_id = query
        .correlation_id
        .clone()
        .or_else(|| {
            rows.first()
                .map(|row| row.evidence().correlation_id.clone())
        })
        .unwrap_or_else(|| "reporting-empty-window".to_string());
    let source = "reporting_service.read_models.v1".to_string();
    let audit_event = ReportingReadAuditEvent {
        dataset: dataset.as_str().to_string(),
        row_count: rows.len(),
        as_of_utc: as_of_utc.clone(),
        source: source.clone(),
        reason_code: reason_code.clone(),
        correlation_id: correlation_id.clone(),
    };

    Ok(ReportingDatasetResponse {
        dataset,
        data_state,
        as_of_utc,
        source,
        reason_code,
        correlation_id,
        start_inclusive_utc: query.start_inclusive_utc.clone(),
        end_exclusive_utc: query.end_exclusive_utc.clone(),
        rows,
        audit_event,
    })
}

fn ensure_evidence_metadata(
    evidence: &ReportingEvidenceMetadata,
) -> Result<(), ReportingReadModelError> {
    if evidence.source.trim().is_empty() {
        return Err(ReportingReadModelError::evidence_unavailable(
            "reporting evidence is missing `source`",
        ));
    }
    if evidence.reason_code.trim().is_empty() {
        return Err(ReportingReadModelError::evidence_unavailable(
            "reporting evidence is missing `reason_code`",
        ));
    }
    if evidence.correlation_id.trim().is_empty() {
        return Err(ReportingReadModelError::evidence_unavailable(
            "reporting evidence is missing `correlation_id`",
        ));
    }
    if parse_utc_timestamp("as_of_utc", &evidence.as_of_utc).is_err() {
        return Err(ReportingReadModelError::evidence_unavailable(
            "reporting evidence has invalid `as_of_utc` timestamp",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::reporting::TradeReportingRow;

    fn sample_request(role: &str) -> ReportingReadRequest {
        ReportingReadRequest {
            actor_role: role.to_string(),
            start_inclusive_utc: "2026-04-07T01:00:00Z".to_string(),
            end_exclusive_utc: "2026-04-07T02:00:00Z".to_string(),
            market_id: Some("market-btc-election".to_string()),
            alpha_id: Some("alpha-momentum".to_string()),
            correlation_id: Some("corr-reporting-001".to_string()),
            limit: DEFAULT_REPORTING_LIMIT,
        }
    }

    fn sample_trade_row(report_row_id: &str, occurred_at_utc: &str) -> TradeReportingRow {
        TradeReportingRow {
            report_row_id: report_row_id.to_string(),
            order_id: "order-001".to_string(),
            trade_id: format!("trade-{report_row_id}"),
            market_id: "market-btc-election".to_string(),
            asset_id: "asset-btc".to_string(),
            lifecycle_state: "filled".to_string(),
            event_status: "matched".to_string(),
            occurred_at_utc: occurred_at_utc.to_string(),
            evidence: ReportingEvidenceMetadata {
                as_of_utc: "2026-04-07T02:00:00Z".to_string(),
                source: "reporting.read-models.v1".to_string(),
                reason_code: "reporting_ready".to_string(),
                correlation_id: "corr-reporting-001".to_string(),
                run_id: Some("run-001".to_string()),
                snapshot_id: Some("snapshot-001".to_string()),
            },
        }
    }

    #[tokio::test]
    async fn query_rejects_unauthorized_roles_before_pool_lookup() {
        let orchestrator = ReportingReadModelOrchestrator::without_pool();
        let error = orchestrator
            .query_trade_rows(&sample_request("guest"))
            .await
            .expect_err("unauthorized role should fail");
        assert_eq!(error.code, ReportingReasonCode::Unauthorized.code());
    }

    #[tokio::test]
    async fn query_fails_closed_when_dependencies_are_unavailable_or_stale() {
        let unavailable = ReportingReadModelOrchestrator::without_pool().with_dependency_status(
            ReportingDependencyStatus {
                projections_available: false,
                projections_fresh: true,
            },
        );
        let unavailable_error = unavailable
            .query_trade_rows(&sample_request("read_only_analytics"))
            .await
            .expect_err("unavailable dependencies should fail closed");
        assert_eq!(
            unavailable_error.code,
            ReportingReasonCode::DependencyUnavailable.code()
        );

        let stale = ReportingReadModelOrchestrator::without_pool().with_dependency_status(
            ReportingDependencyStatus {
                projections_available: true,
                projections_fresh: false,
            },
        );
        let stale_error = stale
            .query_trade_rows(&sample_request("read_only_analytics"))
            .await
            .expect_err("stale dependencies should fail closed");
        assert_eq!(
            stale_error.code,
            ReportingReasonCode::StaleDependency.code()
        );
    }

    #[tokio::test]
    async fn query_fails_closed_when_pool_is_not_initialized() {
        let orchestrator = ReportingReadModelOrchestrator::without_pool();
        let error = orchestrator
            .query_trade_rows(&sample_request("read_only_analytics"))
            .await
            .expect_err("missing pool should fail closed");
        assert_eq!(
            error.code,
            ReportingReasonCode::DependencyUnavailable.code()
        );
    }

    #[tokio::test]
    async fn additional_dataset_queries_propagate_dependency_failures() {
        let orchestrator = ReportingReadModelOrchestrator::without_pool();
        let request = sample_request("read_only_analytics");

        let position_error = orchestrator
            .query_position_rows(&request)
            .await
            .expect_err("position query should fail without pool");
        assert_eq!(
            position_error.code,
            ReportingReasonCode::DependencyUnavailable.code()
        );

        let risk_error = orchestrator
            .query_risk_event_rows(&request)
            .await
            .expect_err("risk-event query should fail without pool");
        assert_eq!(
            risk_error.code,
            ReportingReasonCode::DependencyUnavailable.code()
        );

        let performance_error = orchestrator
            .query_performance_rows(&request)
            .await
            .expect_err("performance query should fail without pool");
        assert_eq!(
            performance_error.code,
            ReportingReasonCode::DependencyUnavailable.code()
        );
    }

    #[test]
    fn response_builder_marks_empty_windows_with_machine_reason_code() {
        let query = sample_request("read_only_analytics")
            .to_query()
            .expect("query should build");
        let response = finalize_dataset_response::<TradeReportingRow>(
            ReportingReadDataset::Trade,
            &query,
            Vec::new(),
        )
        .expect("empty response should be valid");
        assert_eq!(response.data_state, "empty");
        assert_eq!(
            response.reason_code,
            ReportingReasonCode::EmptyWindow.code().to_string()
        );
        assert_eq!(response.as_of_utc, query.end_exclusive_utc);
    }

    #[test]
    fn response_builder_rejects_missing_evidence_metadata() {
        let query = sample_request("read_only_analytics")
            .to_query()
            .expect("query should build");
        let mut row = sample_trade_row("trade-row-001", "2026-04-07T01:59:59Z");
        row.evidence.source = " ".to_string();

        let error = finalize_dataset_response(ReportingReadDataset::Trade, &query, vec![row])
            .expect_err("missing evidence source should fail closed");
        assert_eq!(error.code, ReportingReasonCode::EvidenceUnavailable.code());
    }

    #[test]
    fn request_to_query_rejects_non_utc_offsets() {
        let mut request = sample_request("read_only_analytics");
        request.start_inclusive_utc = "2026-04-07T01:00:00+01:00".to_string();

        let error = request
            .to_query()
            .expect_err("non-UTC timestamp filters should fail");
        assert_eq!(error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "start_inclusive_utc")
        );
    }

    #[test]
    fn request_to_query_rejects_malformed_market_identifier() {
        let mut request = sample_request("read_only_analytics");
        request.market_id = Some("market/btc-election".to_string());

        let error = request
            .to_query()
            .expect_err("malformed market identifier should fail");
        assert_eq!(error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market_id")
        );
    }

    #[test]
    fn response_builder_rejects_invalid_evidence_timestamps() {
        let query = sample_request("read_only_analytics")
            .to_query()
            .expect("query should build");
        let mut row = sample_trade_row("trade-row-001", "2026-04-07T01:59:59Z");
        row.evidence.as_of_utc = "2026-04-07T01:59:59+01:00".to_string();

        let error = finalize_dataset_response(ReportingReadDataset::Trade, &query, vec![row])
            .expect_err("non-UTC evidence timestamps should fail closed");
        assert_eq!(error.code, ReportingReasonCode::EvidenceUnavailable.code());
    }

    #[test]
    fn response_builder_uses_row_correlation_when_filter_is_absent() {
        let mut request = sample_request("read_only_analytics");
        request.correlation_id = None;
        let query = request.to_query().expect("query should build");
        let mut row = sample_trade_row("trade-row-001", "2026-04-07T01:59:59Z");
        row.evidence.correlation_id = "corr-from-row".to_string();

        let response = finalize_dataset_response(ReportingReadDataset::Trade, &query, vec![row])
            .expect("response should build");
        assert_eq!(response.correlation_id, "corr-from-row");
    }

    #[test]
    fn response_builder_preserves_deterministic_top_level_metadata() {
        let query = sample_request("read_only_analytics")
            .to_query()
            .expect("query should build");
        let mut rows = vec![
            sample_trade_row("trade-row-2", "2026-04-07T01:59:59Z"),
            sample_trade_row("trade-row-1", "2026-04-07T02:00:00Z"),
        ];
        sort_trade_reporting_rows(&mut rows);

        let response = finalize_dataset_response(ReportingReadDataset::Trade, &query, rows)
            .expect("response should build");
        assert_eq!(response.data_state, "ready");
        assert_eq!(
            response.reason_code,
            ReportingReasonCode::Ready.code().to_string()
        );
        assert_eq!(response.rows.len(), 2);
        assert_eq!(response.audit_event.dataset, "trade");
        assert_eq!(response.audit_event.row_count, 2);
    }

    #[test]
    fn request_helper_sets_default_limit_for_reporting_queries() {
        let request = ReportingReadRequest::with_default_limit(
            "read_only_analytics",
            "2026-04-07T01:00:00Z",
            "2026-04-07T02:00:00Z",
        );
        assert_eq!(request.limit, DEFAULT_REPORTING_LIMIT);
    }
}
