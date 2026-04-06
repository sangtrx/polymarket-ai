use domain::attribution::{
    AttributionContractError, AttributionObservation, AttributionPeriod, AttributionReasonCode,
    AttributionRow, build_cost_aware_attribution_rows, build_query_scope,
};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct AttributionRuntimeInput {
    pub market_id: String,
    pub alpha_id: String,
    pub period_end_utc: String,
    pub realized_pnl_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub fees_usd: f64,
    pub rebates_usd: f64,
    pub incentives_usd: f64,
    pub as_of_utc: String,
    pub source: String,
    pub reason_code: AttributionReasonCode,
    pub correlation_id: String,
    pub snapshot_id: Option<String>,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AttributionReadModel {
    pub period: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub as_of_utc: String,
    pub source: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub data_state: String,
    pub recommended_next_action: String,
    pub rows: Vec<AttributionRow>,
}

pub fn build_attribution_read_model(
    period: AttributionPeriod,
    as_of_utc: &str,
    market_id: Option<&str>,
    alpha_id: Option<&str>,
    stale_source: bool,
    runtime_inputs: &[AttributionRuntimeInput],
) -> Result<AttributionReadModel, AttributionContractError> {
    if stale_source {
        return Err(AttributionContractError {
            code: AttributionReasonCode::StaleSource.code(),
            message: "reconciliation evidence is stale for attribution projection".to_string(),
            field_errors: Vec::new(),
        });
    }

    let scope = build_query_scope(period, as_of_utc, market_id, alpha_id)?;
    let observations: Vec<AttributionObservation> = runtime_inputs
        .iter()
        .map(|entry| AttributionObservation {
            market_id: entry.market_id.clone(),
            alpha_id: entry.alpha_id.clone(),
            period_end_utc: entry.period_end_utc.clone(),
            realized_pnl_usd: entry.realized_pnl_usd,
            unrealized_pnl_usd: entry.unrealized_pnl_usd,
            fees_usd: entry.fees_usd,
            rebates_usd: entry.rebates_usd,
            incentives_usd: entry.incentives_usd,
            as_of_utc: entry.as_of_utc.clone(),
            source: entry.source.clone(),
            reason_code: entry.reason_code.code().to_string(),
            correlation_id: entry.correlation_id.clone(),
            snapshot_id: entry.snapshot_id.clone(),
            run_id: entry.run_id.clone(),
        })
        .collect();
    let rows = build_cost_aware_attribution_rows(&observations, &scope)?;
    if rows.is_empty() {
        return Ok(AttributionReadModel {
            period: scope.period.as_str().to_string(),
            period_start_utc: scope.start_inclusive_utc.clone(),
            period_end_utc: scope.end_exclusive_utc.clone(),
            as_of_utc: scope.as_of_utc.clone(),
            source: "portfolio-engine.attribution.v1".to_string(),
            reason_code: AttributionReasonCode::EmptyWindow.code().to_string(),
            correlation_id: format!(
                "corr-attribution-empty::{}::{}",
                scope.period.as_str(),
                scope.as_of_utc
            ),
            data_state: "empty".to_string(),
            recommended_next_action:
                "No activity in this period. Expand to a wider window or remove restrictive filters."
                    .to_string(),
            rows,
        });
    }

    Ok(AttributionReadModel {
        period: scope.period.as_str().to_string(),
        period_start_utc: scope.start_inclusive_utc.clone(),
        period_end_utc: scope.end_exclusive_utc.clone(),
        as_of_utc: scope.as_of_utc,
        source: rows[0].source.clone(),
        reason_code: AttributionReasonCode::Ready.code().to_string(),
        correlation_id: rows[0].correlation_id.clone(),
        data_state: "ready".to_string(),
        recommended_next_action:
            "Inspect top net contributors and confirm cost drag against expected execution quality."
                .to_string(),
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_input(
        market_id: &str,
        alpha_id: &str,
        period_end_utc: &str,
        correlation_id: &str,
    ) -> AttributionRuntimeInput {
        AttributionRuntimeInput {
            market_id: market_id.to_string(),
            alpha_id: alpha_id.to_string(),
            period_end_utc: period_end_utc.to_string(),
            realized_pnl_usd: 140.0,
            unrealized_pnl_usd: 20.0,
            fees_usd: 6.0,
            rebates_usd: 2.0,
            incentives_usd: 1.0,
            as_of_utc: "2026-04-06T15:00:00Z".to_string(),
            source: "portfolio-engine.attribution.v1".to_string(),
            reason_code: AttributionReasonCode::Ready,
            correlation_id: correlation_id.to_string(),
            snapshot_id: Some("snapshot::attribution::001".to_string()),
            run_id: Some("run::reconciliation::001".to_string()),
        }
    }

    #[test]
    fn builds_cost_aware_read_model_with_metadata_evidence() {
        let read_model = build_attribution_read_model(
            AttributionPeriod::TwentyFourHours,
            "2026-04-06T15:00:00Z",
            None,
            None,
            false,
            &[runtime_input(
                "market-btc-election",
                "alpha-momentum",
                "2026-04-06T14:30:00Z",
                "corr-attribution-001",
            )],
        )
        .expect("read model should be built");

        assert_eq!(read_model.data_state, "ready");
        assert_eq!(read_model.reason_code, AttributionReasonCode::Ready.code());
        assert_eq!(read_model.rows.len(), 1);
        assert_eq!(read_model.rows[0].as_of_utc, "2026-04-06T15:00:00Z");
        assert_eq!(
            read_model.rows[0].snapshot_id.as_deref(),
            Some("snapshot::attribution::001")
        );
        assert_eq!(
            read_model.rows[0].run_id.as_deref(),
            Some("run::reconciliation::001")
        );
    }

    #[test]
    fn respects_start_inclusive_end_exclusive_boundary_semantics() {
        let read_model = build_attribution_read_model(
            AttributionPeriod::OneHour,
            "2026-04-06T15:00:00Z",
            None,
            None,
            false,
            &[
                runtime_input(
                    "market-btc-election",
                    "alpha-momentum",
                    "2026-04-06T14:00:00Z",
                    "corr-included",
                ),
                runtime_input(
                    "market-btc-election",
                    "alpha-momentum",
                    "2026-04-06T15:00:00Z",
                    "corr-excluded",
                ),
            ],
        )
        .expect("read model should be built");

        assert_eq!(read_model.rows.len(), 1);
        assert_eq!(read_model.rows[0].correlation_id, "corr-included");
    }

    #[test]
    fn empty_window_is_explicit_and_actionable() {
        let read_model = build_attribution_read_model(
            AttributionPeriod::OneHour,
            "2026-04-06T15:00:00Z",
            Some("market-btc-election"),
            None,
            false,
            &[runtime_input(
                "market-btc-election",
                "alpha-momentum",
                "2026-04-06T12:00:00Z",
                "corr-old",
            )],
        )
        .expect("read model should be built");

        assert_eq!(read_model.data_state, "empty");
        assert_eq!(
            read_model.reason_code,
            AttributionReasonCode::EmptyWindow.code()
        );
        assert!(read_model.rows.is_empty());
        assert!(
            read_model
                .recommended_next_action
                .contains("Expand to a wider window")
        );
    }

    #[test]
    fn stale_source_returns_machine_reason_code() {
        let error = build_attribution_read_model(
            AttributionPeriod::TwentyFourHours,
            "2026-04-06T15:00:00Z",
            None,
            None,
            true,
            &[runtime_input(
                "market-btc-election",
                "alpha-momentum",
                "2026-04-06T14:30:00Z",
                "corr-stale",
            )],
        )
        .expect_err("stale source should fail closed");

        assert_eq!(error.code, AttributionReasonCode::StaleSource.code());
    }
}
