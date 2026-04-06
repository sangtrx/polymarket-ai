use crate::postgres::attribution_snapshots::{
    AttributionPersistenceError, load_latest_attribution_snapshots,
};
use crate::postgres::reconciliation::{
    ReconciliationIncidentEvidence, ReconciliationPersistenceError,
    load_reconciliation_incident_evidence,
};
use domain::attribution::{AttributionPeriod, AttributionRow};
use domain::incidents::{
    IncidentContractError, IncidentQueryFilters, IncidentReasonCode, IncidentTimelineEvent,
    IncidentTimelineStage, IncidentValidationIssue, apply_incident_query,
    normalize_incident_identifier, validate_incident_timeline_event,
};
use domain::reconciliation::{
    ReconciliationDiffClass, ReconciliationRunStatus, ReconciliationRunSummary,
};
use sqlx::{PgPool, Row};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::task::JoinSet;

const LOAD_INCIDENT_QUERY_VIEWS_SQL: &str = r#"
    SELECT
        view_id,
        to_char(occurred_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS occurred_at,
        market_id,
        order_id,
        alpha_id,
        actor_id,
        stage,
        source,
        reason_code,
        correlation_id,
        run_id,
        snapshot_id,
        summary,
        severity,
        recommended_next_action
    FROM incident_query_views
    WHERE occurred_at >= $1::timestamptz
      AND occurred_at < $2::timestamptz
      AND ($3::text IS NULL OR market_id = $3)
      AND ($4::text IS NULL OR order_id = $4)
      AND ($5::text IS NULL OR alpha_id = $5)
      AND ($6::text IS NULL OR actor_id = $6)
    ORDER BY occurred_at DESC, view_id ASC
    LIMIT $7
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidentQueryPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<IncidentValidationIssue>,
}

impl IncidentQueryPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<IncidentValidationIssue>,
    ) -> Self {
        Self {
            code: IncidentReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn dependency_unavailable(operation: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: IncidentReasonCode::DependencyUnavailable.code(),
            message: format!("{operation} failed: {}", message.into()),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: IncidentReasonCode::DependencyUnavailable.code(),
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for IncidentQueryPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for IncidentQueryPersistenceError {}

pub async fn load_incident_forensics_timeline(
    pool: &PgPool,
    filters: &IncidentQueryFilters,
) -> Result<Vec<IncidentTimelineEvent>, IncidentQueryPersistenceError> {
    let seeded_events = load_seeded_incident_query_view_rows(pool, filters).await?;
    if !seeded_events.is_empty() {
        return Ok(seeded_events);
    }
    if filters.actor_id.is_some() {
        return Err(IncidentQueryPersistenceError::dependency_unavailable(
            "load_incident_forensics_timeline",
            "actor_id filtering requires seeded incident_query_views evidence",
        ));
    }

    let attribution_period = resolve_attribution_period(filters);
    let attribution_rows = load_latest_attribution_snapshots(
        pool,
        attribution_period,
        filters.market_id.as_deref(),
        filters.alpha_id.as_deref(),
        200,
    )
    .await
    .map_err(map_attribution_error)?;

    let run_ids: BTreeSet<String> = attribution_rows
        .iter()
        .filter_map(|row| row.run_id.clone())
        .collect();
    let mut reconciliation_by_run = BTreeMap::new();
    let mut run_loads = JoinSet::new();
    for run_id in run_ids {
        let pool = pool.clone();
        run_loads.spawn(async move {
            let evidence = load_reconciliation_incident_evidence(&pool, &run_id).await;
            (run_id, evidence)
        });
    }
    while let Some(joined) = run_loads.join_next().await {
        let (run_id, evidence) = joined.map_err(|error| {
            IncidentQueryPersistenceError::dependency_unavailable(
                "load_reconciliation_incident_evidence",
                error.to_string(),
            )
        })?;
        if let Some(evidence) = evidence.map_err(map_reconciliation_error)? {
            reconciliation_by_run.insert(run_id, evidence);
        }
    }

    let events = derive_incident_events_from_evidence(&attribution_rows, &reconciliation_by_run);
    apply_incident_query(&events, filters).map_err(map_contract_error)
}

fn resolve_attribution_period(filters: &IncidentQueryFilters) -> AttributionPeriod {
    let start = OffsetDateTime::parse(&filters.start_ts, &Rfc3339);
    let end = OffsetDateTime::parse(&filters.end_ts, &Rfc3339);
    let Ok(start) = start else {
        return AttributionPeriod::TwentyFourHours;
    };
    let Ok(end) = end else {
        return AttributionPeriod::TwentyFourHours;
    };
    let window = end - start;
    if window <= Duration::hours(1) {
        AttributionPeriod::OneHour
    } else if window <= Duration::hours(24) {
        AttributionPeriod::TwentyFourHours
    } else {
        AttributionPeriod::ThirtyDays
    }
}

async fn load_seeded_incident_query_view_rows(
    pool: &PgPool,
    filters: &IncidentQueryFilters,
) -> Result<Vec<IncidentTimelineEvent>, IncidentQueryPersistenceError> {
    let rows = sqlx::query(LOAD_INCIDENT_QUERY_VIEWS_SQL)
        .bind(&filters.start_ts)
        .bind(&filters.end_ts)
        .bind(filters.market_id.as_deref())
        .bind(filters.order_id.as_deref())
        .bind(filters.alpha_id.as_deref())
        .bind(filters.actor_id.as_deref())
        .bind(500_i64)
        .fetch_all(pool)
        .await
        .map_err(|error| {
            IncidentQueryPersistenceError::dependency_unavailable(
                "load_seeded_incident_query_view_rows",
                error.to_string(),
            )
        })?;
    rows.into_iter().map(decode_seeded_incident_row).collect()
}

fn decode_seeded_incident_row(
    row: sqlx::postgres::PgRow,
) -> Result<IncidentTimelineEvent, IncidentQueryPersistenceError> {
    let stage: String = row
        .try_get("stage")
        .map_err(|error| IncidentQueryPersistenceError::row_decode_failure("stage", error))?;
    let stage = IncidentTimelineStage::parse(&stage).map_err(map_contract_error)?;

    let decoded = IncidentTimelineEvent {
        event_id: row
            .try_get("view_id")
            .map_err(|error| IncidentQueryPersistenceError::row_decode_failure("view_id", error))?,
        occurred_at: row.try_get("occurred_at").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("occurred_at", error)
        })?,
        stage,
        source: row
            .try_get("source")
            .map_err(|error| IncidentQueryPersistenceError::row_decode_failure("source", error))?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("reason_code", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        summary: row
            .try_get("summary")
            .map_err(|error| IncidentQueryPersistenceError::row_decode_failure("summary", error))?,
        recommended_next_action: row.try_get("recommended_next_action").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("recommended_next_action", error)
        })?,
        severity: row.try_get("severity").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("severity", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("market_id", error)
        })?,
        order_id: row.try_get("order_id").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("order_id", error)
        })?,
        alpha_id: row.try_get("alpha_id").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("alpha_id", error)
        })?,
        actor_id: row.try_get("actor_id").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("actor_id", error)
        })?,
        run_id: row
            .try_get("run_id")
            .map_err(|error| IncidentQueryPersistenceError::row_decode_failure("run_id", error))?,
        snapshot_id: row.try_get("snapshot_id").map_err(|error| {
            IncidentQueryPersistenceError::row_decode_failure("snapshot_id", error)
        })?,
    };
    validate_incident_timeline_event(&decoded).map_err(map_contract_error)?;
    Ok(decoded)
}

fn derive_incident_events_from_evidence(
    attribution_rows: &[AttributionRow],
    reconciliation_by_run: &BTreeMap<String, ReconciliationIncidentEvidence>,
) -> Vec<IncidentTimelineEvent> {
    let mut events = Vec::new();
    let mut emitted_runs = BTreeSet::new();
    for row in attribution_rows {
        let base_event_id = row
            .snapshot_id
            .clone()
            .unwrap_or_else(|| format!("attr-{}", row.correlation_id));
        events.push(IncidentTimelineEvent {
            event_id: normalize_incident_identifier(&format!("incident::fill::{base_event_id}")),
            occurred_at: row.period_end_utc.clone(),
            stage: IncidentTimelineStage::Fill,
            source: row.source.clone(),
            reason_code: row.reason_code.clone(),
            correlation_id: row.correlation_id.clone(),
            summary: format!(
                "Fill evidence contributed to attribution for market {} and alpha {}",
                row.market_id, row.alpha_id
            ),
            recommended_next_action: "Verify fill quality and slippage before escalating controls."
                .to_string(),
            severity: attribution_severity(&row.reason_code).to_string(),
            market_id: Some(normalize_incident_identifier(&row.market_id)),
            order_id: None,
            alpha_id: Some(normalize_incident_identifier(&row.alpha_id)),
            actor_id: None,
            run_id: row
                .run_id
                .clone()
                .map(|value| normalize_incident_identifier(&value)),
            snapshot_id: row
                .snapshot_id
                .clone()
                .map(|value| normalize_incident_identifier(&value)),
        });
        events.push(IncidentTimelineEvent {
            event_id: normalize_incident_identifier(&format!("incident::pnl::{base_event_id}")),
            occurred_at: row.period_end_utc.clone(),
            stage: IncidentTimelineStage::Pnl,
            source: row.source.clone(),
            reason_code: row.reason_code.clone(),
            correlation_id: row.correlation_id.clone(),
            summary: format!(
                "PnL attribution row ready for market {} and alpha {}",
                row.market_id, row.alpha_id
            ),
            recommended_next_action:
                "Inspect net-cost impact and validate if mitigation is required.".to_string(),
            severity: attribution_severity(&row.reason_code).to_string(),
            market_id: Some(normalize_incident_identifier(&row.market_id)),
            order_id: None,
            alpha_id: Some(normalize_incident_identifier(&row.alpha_id)),
            actor_id: None,
            run_id: row
                .run_id
                .clone()
                .map(|value| normalize_incident_identifier(&value)),
            snapshot_id: row
                .snapshot_id
                .clone()
                .map(|value| normalize_incident_identifier(&value)),
        });

        if let Some(run_id) = row.run_id.as_deref() {
            let normalized_run_id = normalize_incident_identifier(run_id);
            if emitted_runs.contains(&normalized_run_id) {
                continue;
            }
            let Some(evidence) = reconciliation_by_run.get(&normalized_run_id) else {
                continue;
            };
            emitted_runs.insert(normalized_run_id.clone());
            events.extend(derive_reconciliation_events(&normalized_run_id, evidence));
        }
    }

    events
}

fn derive_reconciliation_events(
    normalized_run_id: &str,
    evidence: &ReconciliationIncidentEvidence,
) -> Vec<IncidentTimelineEvent> {
    let mut events = Vec::new();
    events.push(IncidentTimelineEvent {
        event_id: normalize_incident_identifier(&format!(
            "incident::signal::{}",
            evidence.run.run_id
        )),
        occurred_at: evidence.run.evaluated_at_utc.clone(),
        stage: IncidentTimelineStage::Signal,
        source: "reconciliation.runs.v1".to_string(),
        reason_code: evidence.run.reason_code.clone(),
        correlation_id: evidence.run.correlation_id.clone(),
        summary: format!(
            "Reconciliation signal: {} mismatches over {} compared records",
            evidence.run.mismatch_count, evidence.run.compared_records
        ),
        recommended_next_action: signal_next_action(&evidence.run).to_string(),
        severity: if evidence.run.critical_halt {
            "critical".to_string()
        } else if evidence.run.mismatch_count > 0 {
            "warning".to_string()
        } else {
            "normal".to_string()
        },
        market_id: None,
        order_id: None,
        alpha_id: None,
        actor_id: None,
        run_id: Some(normalized_run_id.to_string()),
        snapshot_id: None,
    });

    for diff in evidence.diffs.iter().take(64) {
        events.push(IncidentTimelineEvent {
            event_id: normalize_incident_identifier(&format!("incident::order::{}", diff.diff_id)),
            occurred_at: diff.observed_at_utc.clone(),
            stage: IncidentTimelineStage::Order,
            source: "reconciliation.diffs.v1".to_string(),
            reason_code: diff.reason_code.clone(),
            correlation_id: diff.correlation_id.clone(),
            summary: format!(
                "Order {} showed {}",
                diff.order_id,
                diff_class_label(diff.diff_class)
            ),
            recommended_next_action:
                "Compare internal and venue order lifecycle values to resolve mismatch.".to_string(),
            severity: if diff.diff_class == ReconciliationDiffClass::LifecycleStateMismatch {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            market_id: Some(normalize_incident_identifier(&diff.market_id)),
            order_id: Some(normalize_incident_identifier(&diff.order_id)),
            alpha_id: None,
            actor_id: None,
            run_id: Some(normalized_run_id.to_string()),
            snapshot_id: None,
        });
    }

    if let Some(snapshot) = evidence.latest_snapshot.as_ref() {
        events.push(IncidentTimelineEvent {
            event_id: normalize_incident_identifier(&format!(
                "incident::risk_action::{}",
                snapshot.snapshot_id
            )),
            occurred_at: snapshot.captured_at_utc.clone(),
            stage: IncidentTimelineStage::RiskAction,
            source: "reconciliation.exposure.v1".to_string(),
            reason_code: snapshot.reason_code.clone(),
            correlation_id: snapshot.correlation_id.clone(),
            summary: format!(
                "Exposure snapshot net {:.4}, gross {:.4}, open orders {}",
                snapshot.net_exposure, snapshot.gross_exposure, snapshot.open_order_count
            ),
            recommended_next_action:
                "Validate containment mode (pause/reduce-only) before resuming flow.".to_string(),
            severity: if snapshot.open_order_count > 0 {
                "warning".to_string()
            } else {
                "normal".to_string()
            },
            market_id: snapshot
                .market_id
                .clone()
                .map(|value| normalize_incident_identifier(&value)),
            order_id: None,
            alpha_id: None,
            actor_id: None,
            run_id: Some(normalized_run_id.to_string()),
            snapshot_id: Some(normalize_incident_identifier(&snapshot.snapshot_id)),
        });
    }

    events
}

fn signal_next_action(run: &ReconciliationRunSummary) -> &'static str {
    if run.status == ReconciliationRunStatus::CriticalHalt {
        "Pause trading and triage reconciliation critical-halt evidence immediately."
    } else if run.mismatch_count > 0 {
        "Investigate mismatch clusters and reconcile order lifecycle state before continuing."
    } else {
        "No blocking mismatch evidence. Continue monitoring correlated timeline events."
    }
}

fn diff_class_label(diff_class: ReconciliationDiffClass) -> &'static str {
    match diff_class {
        ReconciliationDiffClass::MissingInternalRecord => "missing internal record",
        ReconciliationDiffClass::MissingVenueRecord => "missing venue record",
        ReconciliationDiffClass::MarketMismatch => "market mismatch",
        ReconciliationDiffClass::LifecycleStateMismatch => "lifecycle-state mismatch",
        ReconciliationDiffClass::QuantityMismatch => "quantity mismatch",
        ReconciliationDiffClass::PriceMismatch => "price mismatch",
    }
}

fn attribution_severity(reason_code: &str) -> &'static str {
    if reason_code.contains("stale") || reason_code.contains("unavailable") {
        "critical"
    } else if reason_code.contains("empty") {
        "warning"
    } else {
        "normal"
    }
}

fn map_contract_error(error: IncidentContractError) -> IncidentQueryPersistenceError {
    IncidentQueryPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn map_reconciliation_error(
    error: ReconciliationPersistenceError,
) -> IncidentQueryPersistenceError {
    IncidentQueryPersistenceError::dependency_unavailable(
        "load_reconciliation_incident_evidence",
        error.message,
    )
}

fn map_attribution_error(error: AttributionPersistenceError) -> IncidentQueryPersistenceError {
    let field_errors = error
        .field_errors
        .into_iter()
        .map(|issue| IncidentValidationIssue {
            field: issue.field,
            code: issue.code,
            message: issue.message,
        })
        .collect::<Vec<_>>();
    if field_errors.is_empty() {
        IncidentQueryPersistenceError::dependency_unavailable(
            "load_latest_attribution_snapshots",
            error.message,
        )
    } else {
        IncidentQueryPersistenceError::invalid_payload(error.message, field_errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::attribution::{AttributionCostBreakdown, AttributionReasonCode};
    use domain::reconciliation::{
        ExposureSnapshot, ReconciliationDiffRecord, ReconciliationReasonCode,
    };

    const INCIDENT_QUERY_VIEWS_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406193000_incident_query_views.sql");

    fn sample_attribution_row() -> AttributionRow {
        AttributionRow {
            market_id: "market-btc-election".to_string(),
            alpha_id: "alpha-momentum".to_string(),
            period: "24h".to_string(),
            period_start_utc: "2026-04-06T14:00:00Z".to_string(),
            period_end_utc: "2026-04-06T14:30:00Z".to_string(),
            realized_pnl_usd: 120.0,
            unrealized_pnl_usd: 30.0,
            gross_pnl_usd: 150.0,
            net_pnl_usd: 144.0,
            costs: AttributionCostBreakdown {
                fees_usd: 8.0,
                rebates_usd: 2.0,
                incentives_usd: 0.0,
                net_cost_impact_usd: 6.0,
            },
            as_of_utc: "2026-04-06T15:00:00Z".to_string(),
            source: "reconciliation.exposure.v1".to_string(),
            reason_code: AttributionReasonCode::Ready.code().to_string(),
            correlation_id: "corr-attribution-001".to_string(),
            snapshot_id: Some("snapshot-1".to_string()),
            run_id: Some("run-1".to_string()),
        }
    }

    fn sample_reconciliation_evidence() -> ReconciliationIncidentEvidence {
        ReconciliationIncidentEvidence {
            run: ReconciliationRunSummary {
                run_id: "run-1".to_string(),
                window_started_at_utc: "2026-04-06T14:00:00Z".to_string(),
                window_ended_at_utc: "2026-04-06T14:30:00Z".to_string(),
                compared_records: 12,
                mismatch_count: 2,
                mismatch_rate: 0.1666666666,
                critical_halt: false,
                status: ReconciliationRunStatus::Succeeded,
                reason_code: ReconciliationReasonCode::NonCriticalMismatch
                    .code()
                    .to_string(),
                evaluated_at_utc: "2026-04-06T14:31:00Z".to_string(),
                correlation_id: "corr-recon-001".to_string(),
            },
            diffs: vec![ReconciliationDiffRecord {
                diff_id: "diff-run-1-order-1".to_string(),
                run_id: "run-1".to_string(),
                order_id: "order-1".to_string(),
                market_id: "market-btc-election".to_string(),
                diff_class: ReconciliationDiffClass::LifecycleStateMismatch,
                reason_code: ReconciliationReasonCode::NonCriticalMismatch
                    .code()
                    .to_string(),
                internal_value: Some("open".to_string()),
                venue_value: Some("filled".to_string()),
                observed_at_utc: "2026-04-06T14:30:30Z".to_string(),
                correlation_id: "corr-recon-001".to_string(),
            }],
            latest_snapshot: Some(ExposureSnapshot {
                snapshot_id: "snapshot-1".to_string(),
                run_id: "run-1".to_string(),
                market_id: Some("market-btc-election".to_string()),
                net_exposure: 1.1,
                gross_exposure: 1.8,
                open_order_count: 2,
                reason_code: ReconciliationReasonCode::NonCriticalMismatch
                    .code()
                    .to_string(),
                captured_at_utc: "2026-04-06T14:31:30Z".to_string(),
                correlation_id: "corr-recon-001".to_string(),
            }),
        }
    }

    #[test]
    fn migration_scope_remains_limited_to_incident_query_views() {
        assert!(
            INCIDENT_QUERY_VIEWS_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS incident_query_views")
        );
        assert!(!INCIDENT_QUERY_VIEWS_MIGRATION_SQL.contains("reconciliation_runs"));
        assert!(!INCIDENT_QUERY_VIEWS_MIGRATION_SQL.contains("attribution_snapshots"));
        assert!(!INCIDENT_QUERY_VIEWS_MIGRATION_SQL.contains("risk_limit_profiles"));
    }

    #[test]
    fn migration_enforces_canonical_constraints_and_indexes() {
        assert!(
            INCIDENT_QUERY_VIEWS_MIGRATION_SQL
                .contains("stage IN ('signal', 'order', 'fill', 'pnl', 'risk_action')")
        );
        assert!(
            INCIDENT_QUERY_VIEWS_MIGRATION_SQL
                .contains("severity IN ('normal', 'warning', 'critical', 'degraded')")
        );
        assert!(
            INCIDENT_QUERY_VIEWS_MIGRATION_SQL.contains("idx_incident_query_views_window_lookup")
        );
        assert!(
            INCIDENT_QUERY_VIEWS_MIGRATION_SQL.contains("idx_incident_query_views_market_window")
        );
        assert!(
            INCIDENT_QUERY_VIEWS_MIGRATION_SQL.contains("idx_incident_query_views_order_window")
        );
        assert!(
            INCIDENT_QUERY_VIEWS_MIGRATION_SQL.contains("idx_incident_query_views_alpha_window")
        );
        assert!(
            INCIDENT_QUERY_VIEWS_MIGRATION_SQL.contains("idx_incident_query_views_actor_window")
        );
        assert!(
            INCIDENT_QUERY_VIEWS_MIGRATION_SQL
                .contains("idx_incident_query_views_correlation_time")
        );
    }

    #[test]
    fn seeded_query_sql_enforces_deterministic_order_and_boundaries() {
        assert!(LOAD_INCIDENT_QUERY_VIEWS_SQL.contains("occurred_at >= $1::timestamptz"));
        assert!(LOAD_INCIDENT_QUERY_VIEWS_SQL.contains("occurred_at < $2::timestamptz"));
        assert!(LOAD_INCIDENT_QUERY_VIEWS_SQL.contains("ORDER BY occurred_at DESC, view_id ASC"));
    }

    #[test]
    fn derived_events_cover_signal_order_fill_pnl_and_risk_action_stages() {
        let attribution = vec![sample_attribution_row()];
        let mut by_run = BTreeMap::new();
        by_run.insert("run-1".to_string(), sample_reconciliation_evidence());
        let events = derive_incident_events_from_evidence(&attribution, &by_run);

        assert!(
            events
                .iter()
                .any(|event| event.stage == IncidentTimelineStage::Signal)
        );
        assert!(
            events
                .iter()
                .any(|event| event.stage == IncidentTimelineStage::Order)
        );
        assert!(
            events
                .iter()
                .any(|event| event.stage == IncidentTimelineStage::Fill)
        );
        assert!(
            events
                .iter()
                .any(|event| event.stage == IncidentTimelineStage::Pnl)
        );
        assert!(
            events
                .iter()
                .any(|event| event.stage == IncidentTimelineStage::RiskAction)
        );
    }

    #[test]
    fn attribution_period_resolution_follows_requested_query_window() {
        let one_hour = resolve_attribution_period(&IncidentQueryFilters {
            market_id: None,
            order_id: None,
            alpha_id: None,
            actor_id: None,
            start_ts: "2026-04-06T14:00:00Z".to_string(),
            end_ts: "2026-04-06T15:00:00Z".to_string(),
        });
        assert_eq!(one_hour, AttributionPeriod::OneHour);

        let day_window = resolve_attribution_period(&IncidentQueryFilters {
            market_id: None,
            order_id: None,
            alpha_id: None,
            actor_id: None,
            start_ts: "2026-04-06T00:00:00Z".to_string(),
            end_ts: "2026-04-06T12:00:00Z".to_string(),
        });
        assert_eq!(day_window, AttributionPeriod::TwentyFourHours);

        let month_window = resolve_attribution_period(&IncidentQueryFilters {
            market_id: None,
            order_id: None,
            alpha_id: None,
            actor_id: None,
            start_ts: "2026-04-01T00:00:00Z".to_string(),
            end_ts: "2026-04-06T12:00:00Z".to_string(),
        });
        assert_eq!(month_window, AttributionPeriod::ThirtyDays);
    }
}
