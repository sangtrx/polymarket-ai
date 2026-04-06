CREATE OR REPLACE VIEW reporting_trade_read_models AS
SELECT
    lower(trim(user_events.event_id)) AS report_row_id,
    lower(trim(user_events.order_id)) AS order_id,
    COALESCE(lower(trim(user_events.trade_id)), lower(trim(user_events.event_id))) AS trade_id,
    lower(trim(user_events.market_id)) AS market_id,
    lower(trim(user_events.asset_id)) AS asset_id,
    COALESCE(
        lower(trim(latest_transition.to_state)),
        lower(trim(orders.lifecycle_state)),
        lower(trim(user_events.event_status))
    ) AS lifecycle_state,
    lower(trim(user_events.event_status)) AS event_status,
    user_events.event_timestamp_utc AS occurred_at_utc,
    user_events.observed_at_utc AS as_of_utc,
    'user_stream_events'::text AS source,
    lower(trim(user_events.reason_code)) AS reason_code,
    lower(trim(user_events.correlation_id)) AS correlation_id,
    NULL::text AS run_id,
    NULL::text AS snapshot_id
FROM user_stream_events AS user_events
LEFT JOIN orders
    ON orders.order_id = user_events.order_id
LEFT JOIN LATERAL (
    SELECT transition.to_state
    FROM order_state_transitions AS transition
    WHERE transition.order_id = user_events.order_id
    ORDER BY transition.transition_sequence DESC
    LIMIT 1
) AS latest_transition ON TRUE
WHERE user_events.event_kind = 'trade';

CREATE OR REPLACE VIEW reporting_position_read_models AS
SELECT
    lower(trim(snapshot.snapshot_id)) AS report_row_id,
    COALESCE(lower(trim(snapshot.market_id)), 'global') AS market_id,
    snapshot.net_exposure,
    snapshot.gross_exposure,
    snapshot.open_order_count,
    lower(trim(run.status)) AS run_status,
    run.window_started_at_utc,
    run.window_ended_at_utc,
    snapshot.captured_at_utc AS as_of_utc,
    'reconciliation_exposure_snapshots'::text AS source,
    lower(trim(snapshot.reason_code)) AS reason_code,
    lower(trim(snapshot.correlation_id)) AS correlation_id,
    lower(trim(snapshot.run_id)) AS run_id,
    lower(trim(snapshot.snapshot_id)) AS snapshot_id
FROM exposure_snapshots AS snapshot
JOIN reconciliation_runs AS run
    ON run.run_id = snapshot.run_id;

CREATE OR REPLACE VIEW reporting_risk_event_read_models AS
SELECT
    lower(trim(decisions.decision_id)) AS report_row_id,
    'pretrade_gate'::text AS event_type,
    CASE
        WHEN decisions.outcome = 'deny' THEN 'critical'
        ELSE 'normal'
    END::text AS severity,
    lower(trim(decisions.outcome)) AS outcome,
    lower(trim(decisions.market_id)) AS market_id,
    decisions.evaluated_at_utc AS event_at_utc,
    decisions.evaluated_at_utc AS as_of_utc,
    'pretrade_gate_decisions'::text AS source,
    lower(trim(decisions.reason_code)) AS reason_code,
    lower(trim(decisions.correlation_id)) AS correlation_id,
    NULL::text AS run_id,
    NULL::text AS snapshot_id
FROM pretrade_gate_decisions AS decisions
UNION ALL
SELECT
    lower(trim(freshness.event_id)) AS report_row_id,
    'freshness_gate'::text AS event_type,
    CASE
        WHEN freshness.pause_active THEN 'critical'
        WHEN freshness.transition = 'recovery_pending' THEN 'warning'
        ELSE 'normal'
    END::text AS severity,
    lower(trim(freshness.transition)) AS outcome,
    NULL::text AS market_id,
    freshness.evaluated_at_utc AS event_at_utc,
    freshness.evaluated_at_utc AS as_of_utc,
    'freshness_gate_events'::text AS source,
    lower(trim(freshness.reason_code)) AS reason_code,
    lower(trim(freshness.correlation_id)) AS correlation_id,
    NULL::text AS run_id,
    NULL::text AS snapshot_id
FROM freshness_gate_events AS freshness
UNION ALL
SELECT
    lower(trim(control.action_id)) AS report_row_id,
    'safety_control'::text AS event_type,
    CASE
        WHEN control.resulting_mode IN ('paused', 'reduce_only') THEN 'critical'
        ELSE 'warning'
    END::text AS severity,
    lower(trim(control.action)) AS outcome,
    NULL::text AS market_id,
    control.effective_at_utc AS event_at_utc,
    control.effective_at_utc AS as_of_utc,
    'safety_control_actions'::text AS source,
    lower(trim(control.reason_code)) AS reason_code,
    lower(trim(control.correlation_id)) AS correlation_id,
    NULL::text AS run_id,
    NULL::text AS snapshot_id
FROM safety_control_actions AS control
UNION ALL
SELECT
    lower(trim(recovery.run_id)) AS report_row_id,
    'recovery_gate'::text AS event_type,
    CASE
        WHEN recovery.readiness_status = 'blocked' THEN 'critical'
        ELSE 'normal'
    END::text AS severity,
    lower(trim(recovery.readiness_status)) AS outcome,
    NULL::text AS market_id,
    recovery.evaluated_at_utc AS event_at_utc,
    recovery.evaluated_at_utc AS as_of_utc,
    'recovery_gate_runs'::text AS source,
    lower(trim(recovery.reason_code)) AS reason_code,
    lower(trim(recovery.correlation_id)) AS correlation_id,
    lower(trim(recovery.run_id)) AS run_id,
    NULL::text AS snapshot_id
FROM recovery_gate_runs AS recovery
UNION ALL
SELECT
    lower(trim(incident.view_id)) AS report_row_id,
    concat('incident_', lower(trim(incident.stage))) AS event_type,
    lower(trim(incident.severity)) AS severity,
    lower(trim(incident.stage)) AS outcome,
    NULLIF(lower(trim(incident.market_id)), '') AS market_id,
    incident.occurred_at AS event_at_utc,
    incident.occurred_at AS as_of_utc,
    lower(trim(incident.source)) AS source,
    lower(trim(incident.reason_code)) AS reason_code,
    lower(trim(incident.correlation_id)) AS correlation_id,
    NULLIF(lower(trim(incident.run_id)), '') AS run_id,
    NULLIF(lower(trim(incident.snapshot_id)), '') AS snapshot_id
FROM incident_query_views AS incident;

CREATE OR REPLACE VIEW reporting_performance_read_models AS
SELECT
    lower(trim(attribution.snapshot_id)) AS report_row_id,
    lower(trim(attribution.market_id)) AS market_id,
    lower(trim(attribution.alpha_id)) AS alpha_id,
    attribution.period_scope,
    attribution.period_start_utc,
    attribution.period_end_utc,
    attribution.realized_pnl_usd,
    attribution.unrealized_pnl_usd,
    attribution.gross_pnl_usd,
    attribution.net_pnl_usd,
    attribution.fees_usd,
    attribution.rebates_usd,
    attribution.incentives_usd,
    attribution.as_of_utc,
    'attribution_snapshots'::text AS source,
    lower(trim(attribution.reason_code)) AS reason_code,
    lower(trim(attribution.correlation_id)) AS correlation_id,
    lower(trim(attribution.run_id)) AS run_id,
    lower(trim(attribution.snapshot_id)) AS snapshot_id
FROM attribution_snapshots AS attribution;

CREATE INDEX IF NOT EXISTS idx_user_stream_events_reporting_trade_window
    ON user_stream_events (event_timestamp_utc DESC, market_id ASC, order_id ASC, event_id ASC)
    WHERE event_kind = 'trade';

CREATE INDEX IF NOT EXISTS idx_user_stream_events_reporting_trade_correlation_window
    ON user_stream_events (correlation_id, event_timestamp_utc DESC, event_id ASC)
    WHERE event_kind = 'trade';

CREATE INDEX IF NOT EXISTS idx_exposure_snapshots_reporting_position_window
    ON exposure_snapshots (captured_at_utc DESC, market_id ASC, snapshot_id ASC);

CREATE INDEX IF NOT EXISTS idx_exposure_snapshots_reporting_position_correlation
    ON exposure_snapshots (correlation_id, captured_at_utc DESC, snapshot_id ASC);

CREATE INDEX IF NOT EXISTS idx_attribution_snapshots_reporting_performance_window
    ON attribution_snapshots (as_of_utc DESC, market_id ASC, alpha_id ASC, snapshot_id ASC);

CREATE INDEX IF NOT EXISTS idx_attribution_snapshots_reporting_performance_correlation
    ON attribution_snapshots (correlation_id, as_of_utc DESC, snapshot_id ASC);

CREATE INDEX IF NOT EXISTS idx_pretrade_gate_decisions_reporting_risk_window
    ON pretrade_gate_decisions (evaluated_at_utc DESC, decision_id ASC);

CREATE INDEX IF NOT EXISTS idx_freshness_gate_events_reporting_risk_window
    ON freshness_gate_events (evaluated_at_utc DESC, event_id ASC);

CREATE INDEX IF NOT EXISTS idx_safety_control_actions_reporting_risk_window
    ON safety_control_actions (effective_at_utc DESC, action_id ASC);

CREATE INDEX IF NOT EXISTS idx_recovery_gate_runs_reporting_risk_window
    ON recovery_gate_runs (evaluated_at_utc DESC, run_id ASC);

CREATE INDEX IF NOT EXISTS idx_incident_query_views_reporting_risk_window
    ON incident_query_views (occurred_at DESC, view_id ASC);
