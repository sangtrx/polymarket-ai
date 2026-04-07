import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.7 migration scope stays isolated to alpha_health_metrics and alpha_threshold_breaches", () => {
  const migration = read(
    "crates/persistence/migrations/20260408023000_alpha_health_metrics_threshold_breaches.sql",
  );

  assert.match(migration, /CREATE TABLE IF NOT EXISTS alpha_health_metrics/);
  assert.match(migration, /CREATE TABLE IF NOT EXISTS alpha_threshold_breaches/);
  assert.match(
    migration,
    /CHECK \(EXTRACT\(TIMEZONE FROM recorded_at_utc\) = 0\)/,
  );
  assert.match(
    migration,
    /CHECK \(EXTRACT\(TIMEZONE FROM breached_at_utc\) = 0\)/,
  );
  assert.match(migration, /idx_alpha_health_metrics_alpha_lookup/);
  assert.match(migration, /idx_alpha_threshold_breaches_alpha_lookup/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS validation_runs/);
  assert.doesNotMatch(
    migration,
    /CREATE TABLE IF NOT EXISTS counterfactual_replay_runs/,
  );
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS promotion_decisions/);
});

test("Story 6.7 runbook captures FR10 routes, boundary semantics, fail-closed statuses, and cross-links", () => {
  const runbook = read(
    "docs/operations/alpha-live-health-monitoring-threshold-breaches.md",
  );

  assert.match(runbook, /POST \/control\/research\/alpha-health-metrics/);
  assert.match(
    runbook,
    /GET \/control\/research\/alpha-health-metrics\/\{metric_id\}/,
  );
  assert.match(
    runbook,
    /GET \/control\/research\/alpha-health-metrics\?alpha_id=\{alpha_id\}/,
  );
  assert.match(
    runbook,
    /GET \/control\/research\/alpha-threshold-breaches\/\{breach_id\}/,
  );
  assert.match(
    runbook,
    /GET \/control\/research\/alpha-threshold-breaches\?alpha_id=\{alpha_id\}/,
  );
  assert.match(runbook, /rolling_sharpe < threshold_value/);
  assert.match(runbook, /rolling_drawdown > threshold_value/);
  assert.match(runbook, /equality follows allow-path/);
  assert.match(runbook, /alpha_health_invalid_payload/);
  assert.match(runbook, /alpha_health_dependency_unavailable/);
  assert.match(runbook, /alpha-shadow-mode-evaluation\.md/);
  assert.match(runbook, /alpha-counterfactual-replay-stress-gating\.md/);
  assert.match(runbook, /alpha-promotion-lifecycle-governance\.md/);
});

test("Story 6.7 domain contract encodes deterministic FR10 thresholds and required windows", () => {
  const domain = read("crates/domain/src/research.rs");

  assert.match(domain, /pub const FR10_ALPHA_HEALTH_REQUIRED_WINDOWS/);
  assert.match(
    domain,
    /AlphaHealthMetricWindow::OneHour[\s\S]*AlphaHealthMetricWindow::TwentyFourHours[\s\S]*AlphaHealthMetricWindow::ThirtyDays/s,
  );
  assert.match(domain, /pub fn evaluate_alpha_health_thresholds\(/);
  assert.match(domain, /if metric_key\.is_floor_metric\(\) \{/);
  assert.match(domain, /observed_value < threshold_value/);
  assert.match(domain, /observed_value > threshold_value/);
  assert.match(domain, /AlphaHealthReasonCode::ThresholdBreachDetected/);
});

test("Story 6.7 exact-boundary thresholds keep equality on the allow path", () => {
  const domain = read("crates/domain/src/research.rs");

  assert.match(
    domain,
    /fn alpha_health_threshold_boundaries_apply_floor_and_ceiling_semantics\(/,
  );
  assert.match(
    domain,
    /metric\.rolling_sharpe = 1\.0;[\s\S]*metric\.rolling_hit_rate = 0\.5;[\s\S]*metric\.stability_score = 0\.8;[\s\S]*metric\.rolling_drawdown = 0\.1;/s,
  );
  assert.match(domain, /expect\("exact boundary should stay on allow path"\)/);
  assert.match(domain, /assert!\(breaches\.is_empty\(\)\);/);
});

test("Story 6.7 persistence adapter enforces deterministic ordering and boundary-safe filters", () => {
  const persistence = read(
    "crates/persistence/src/postgres/alpha_health_metrics.rs",
  );

  assert.match(persistence, /ORDER BY recorded_at_utc DESC, metric_id ASC/);
  assert.match(persistence, /ORDER BY breached_at_utc DESC, breach_id ASC/);
  assert.match(
    persistence,
    /recorded_before_utc must be greater than recorded_after_utc/,
  );
  assert.match(
    persistence,
    /breached_before_utc must be greater than breached_after_utc/,
  );
  assert.match(persistence, /parse_alpha_health_utc_timestamp\(/);
});

test("Story 6.7 orchestration reuses Story 6.4+6.6 seams and emits alpha-health telemetry", () => {
  const alphaHealth = read(
    "services/research-gateway/src/promotion/alpha_health.rs",
  );

  assert.match(alphaHealth, /pub trait LiveMonitoringContextPort: Send \+ Sync/);
  assert.match(
    alphaHealth,
    /pg_list_promotion_decisions\(&self\.pool, alpha_id, None, None, 1\)/,
  );
  assert.match(
    alphaHealth,
    /pg_list_counterfactual_replay_runs\([\s\S]*&self\.pool, alpha_id, None, None, 1,[\s\S]*\)/s,
  );
  assert.match(
    alphaHealth,
    /pg_list_shadow_evaluations\([\s\S]*&self\.pool, alpha_id, None, None, 1,[\s\S]*\)/s,
  );
  assert.match(alphaHealth, /emit_alpha_health_telemetry\(/);
  assert.match(alphaHealth, /maybe_emit_breach_alert\(/);
});

test("Story 6.7 breach alert payloads reuse incident-alert contract fields", () => {
  const alphaHealth = read(
    "services/research-gateway/src/promotion/alpha_health.rs",
  );

  assert.match(alphaHealth, /fn alpha_health_alert_for_breach\(/);
  assert.match(alphaHealth, /severity: if breach\.metric_key ==/);
  assert.match(
    alphaHealth,
    /reason_code: AlertReasonCode::AlphaHealthThresholdBreach,/,
  );
  assert.match(
    alphaHealth,
    /impacted_subsystem: "alpha_health_monitoring"\.to_string\(\),/,
  );
  assert.match(
    alphaHealth,
    /recommended_next_action:[\s\S]*Review live alpha telemetry and apply promotion lifecycle controls if degradation persists\./s,
  );
  assert.match(
    alphaHealth,
    /build_incident_alert\([\s\S]*&breach\.correlation_id,[\s\S]*ALPHA_HEALTH_EVIDENCE_LINK/s,
  );
});

test("Story 6.7 control-api route tests include alpha-health success and fail-closed coverage", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /alpha_health_metric_start_route_returns_data_meta_error_envelope/);
  assert.match(routes, /alpha_health_read_list_routes_return_envelope_shapes/);
  assert.match(routes, /alpha_health_start_route_maps_json_rejection_to_bad_request_envelope/);
  assert.match(routes, /alpha_health_list_route_maps_query_rejection_to_bad_request_envelope/);
  assert.match(routes, /alpha_health_service_errors_emit_unauthorized_security_signals/);
  assert.match(routes, /alpha_health_dependency_unavailable_maps_to_service_unavailable/);
});

test("Story 6.7 persistence and alert seams keep breach writes atomic and per-breach alerts deduped safely", () => {
  const persistence = read("crates/persistence/src/postgres/alpha_health_metrics.rs");
  const alphaHealth = read(
    "services/research-gateway/src/promotion/alpha_health.rs",
  );

  assert.match(persistence, /upsert_alpha_health_metric_with_breaches/);
  assert.match(persistence, /pool\.begin\(\)/);
  assert.match(persistence, /transaction\.commit\(\)/);
  assert.match(alphaHealth, /upsert_metric_and_breaches/);
  assert.match(alphaHealth, /alpha_health_alert_correlation_key/);
});

test("Story 6.7 QA command wiring executes Rust and story-scoped API/E2E checks", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-6-7"/);
  assert.match(
    packageJson,
    /cargo test -p domain research::tests::alpha_health_/,
  );
  assert.match(
    packageJson,
    /cargo test -p persistence postgres::alpha_health_metrics::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p research-gateway promotion::alpha_health::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p control-api routes::tests::alpha_health_/,
  );
  assert.match(packageJson, /tests\/api\/story-6-7\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-6-7\*\.test\.mjs/);
});
