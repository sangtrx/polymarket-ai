import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.7 exposes authenticated alpha-health metric and breach routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/research\/alpha-health-metrics"/);
  assert.match(
    routes,
    /post\(start_alpha_health_metric\)\.get\(list_alpha_health_metrics\)/,
  );
  assert.match(
    routes,
    /\/control\/research\/alpha-health-metrics\/\{metric_id\}"/,
  );
  assert.match(routes, /get\(read_alpha_health_metric\)/);
  assert.match(routes, /\/control\/research\/alpha-threshold-breaches"/);
  assert.match(routes, /get\(list_alpha_threshold_breaches\)/);
  assert.match(
    routes,
    /\/control\/research\/alpha-threshold-breaches\/\{breach_id\}"/,
  );
  assert.match(routes, /get\(read_alpha_threshold_breach\)/);
});

test("Story 6.7 payload/query/response contracts follow canonical envelopes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct AlphaHealthMetricStartPayload \{[\s\S]*pub alpha_id: String,[\s\S]*pub rolling_sharpe: f64,[\s\S]*pub rolling_hit_rate: f64,[\s\S]*pub rolling_drawdown: f64,[\s\S]*pub stability_score: f64,[\s\S]*pub windows: Vec<domain::research::AlphaHealthAttributionWindowMetrics>,[\s\S]*pub thresholds: Vec<domain::research::AlphaHealthThresholdDefinition>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct AlphaHealthMetricsQuery \{[\s\S]*pub alpha_id: String,[\s\S]*pub limit: Option<i64>,[\s\S]*pub recorded_after_utc: Option<String>,[\s\S]*pub recorded_before_utc: Option<String>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct AlphaThresholdBreachesQuery \{[\s\S]*pub alpha_id: String,[\s\S]*pub limit: Option<i64>,[\s\S]*pub breached_after_utc: Option<String>,[\s\S]*pub breached_before_utc: Option<String>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct AlphaHealthEnvelope<T: Serialize> \{[\s\S]*pub data: Option<T>,[\s\S]*pub meta: AlphaHealthMeta,[\s\S]*pub error: Option<AlphaHealthEnvelopeError>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub enum AlphaHealthData \{[\s\S]*Metric[\s\S]*Metrics[\s\S]*Breach[\s\S]*Breaches[\s\S]*\}/s,
  );
  assert.match(routes, /pub struct AlphaHealthEnvelopeError \{/);
});

test("Story 6.7 alpha-health status mapping is deterministic and fail-closed", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn alpha_health_service_error_status\(code: &str\) -> StatusCode/,
  );
  assert.match(
    routes,
    /AlphaHealthReasonCode::InvalidPayload\.code\(\)[\s\S]*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /AlphaHealthReasonCode::UnauthorizedRole\.code\(\)[\s\S]*StatusCode::FORBIDDEN/s,
  );
  assert.match(
    routes,
    /AlphaHealthReasonCode::MetricNotFound\.code\(\)[\s\S]*AlphaHealthReasonCode::BreachNotFound\.code\(\)[\s\S]*StatusCode::NOT_FOUND/s,
  );
  assert.match(
    routes,
    /"alpha_health_constraint_violation"[\s\S]*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /AlphaHealthReasonCode::DependencyUnavailable\.code\(\)[\s\S]*AlphaHealthReasonCode::StateUnavailable\.code\(\)[\s\S]*AlphaHealthReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 6.7 control-api state/startup wiring includes alpha-health orchestration", () => {
  const middleware = read("services/control-api/src/middleware/mod.rs");
  const main = read("services/control-api/src/main.rs");

  assert.match(
    middleware,
    /pub research_alpha_health_orchestrator: Arc<dyn AlphaHealthOrchestrator>/,
  );
  assert.match(middleware, /with_research_alpha_health_orchestrator\(/);
  assert.match(main, /AlphaHealthService::postgres\(/);
  assert.match(main, /with_research_alpha_health_orchestrator\(/);
});

test("Story 6.7 research-gateway exports alpha-health orchestration seams", () => {
  const promotionMod = read("services/research-gateway/src/promotion/mod.rs");
  const alphaHealth = read(
    "services/research-gateway/src/promotion/alpha_health.rs",
  );

  assert.match(promotionMod, /pub mod alpha_health;/);
  assert.match(alphaHealth, /pub trait AlphaHealthOrchestrator: Send \+ Sync/);
  assert.match(alphaHealth, /fn start_alpha_health_metric\(/);
  assert.match(alphaHealth, /fn read_alpha_health_metric\(/);
  assert.match(alphaHealth, /fn list_alpha_health_metrics\(/);
  assert.match(alphaHealth, /fn read_alpha_threshold_breach\(/);
  assert.match(alphaHealth, /fn list_alpha_threshold_breaches\(/);
});

test("Story 6.7 unauthorized denials emit machine-readable alpha-health security signals", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /unauthorized_alpha_health_read_attempt_v1/);
  assert.match(routes, /unauthorized_alpha_health_mutation_attempt_v1/);
});

test("Story 6.7 list routes canonicalize alpha ids and preserve correlation continuity", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /let effective_correlation_id = query[\s\S]*unwrap_or_else\(\|\| actor\.correlation_id\.clone\(\)\);/s,
  );
  assert.match(
    routes,
    /let canonical_alpha_id = normalize_research_identifier\(&alpha_id\);/,
  );
  assert.match(
    routes,
    /list_alpha_health_metrics\(ListAlphaHealthMetricsInput \{[\s\S]*alpha_id,[\s\S]*correlation_id: effective_correlation_id\.clone\(\),[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /list_alpha_threshold_breaches\(ListAlphaThresholdBreachesInput \{[\s\S]*alpha_id,[\s\S]*correlation_id: effective_correlation_id\.clone\(\),[\s\S]*\}/s,
  );
});

test("Story 6.7 route-level negative-path coverage includes alpha-health envelope guards", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /alpha_health_start_route_maps_json_rejection_to_bad_request_envelope/,
  );
  assert.match(
    routes,
    /alpha_health_list_route_maps_query_rejection_to_bad_request_envelope/,
  );
  assert.match(
    routes,
    /alpha_health_service_errors_emit_unauthorized_security_signals/,
  );
  assert.match(
    routes,
    /alpha_health_dependency_unavailable_maps_to_service_unavailable/,
  );
});

test("Story 6.7 API/runtime seams assert breach payload fields and alert reason continuity", () => {
  const routes = read("services/control-api/src/routes/mod.rs");
  const alphaHealth = read(
    "services/research-gateway/src/promotion/alpha_health.rs",
  );

  assert.match(
    routes,
    /alpha_health_metric_start_route_returns_data_meta_error_envelope/,
  );
  assert.match(
    routes,
    /payload\["data"\]\["breaches"\]\[0\]\["breach_reason"\]/,
  );
  assert.match(alphaHealth, /alpha_health_start_records_breaches_and_emits_alerts/);
  assert.match(
    alphaHealth,
    /AlertReasonCode::AlphaHealthThresholdBreach\.code\(\)/,
  );
});

test("Story 6.7 breach DTO adapters preserve required threshold-record fields", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct AlphaThresholdBreachItem \{[\s\S]*pub alpha_id: String,[\s\S]*pub metric_key: String,[\s\S]*pub comparator: String,[\s\S]*pub observed_value: f64,[\s\S]*pub threshold_value: f64,[\s\S]*pub breach_reason: String,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /fn alpha_threshold_breach_to_item\([\s\S]*alpha_id: breach\.alpha_id,[\s\S]*metric_key: breach\.metric_key\.as_str\(\)\.to_string\(\),[\s\S]*comparator: breach\.comparator\.as_str\(\)\.to_string\(\),[\s\S]*observed_value: breach\.observed_value,[\s\S]*threshold_value: breach\.threshold_value,[\s\S]*breach_reason: breach\.breach_reason,[\s\S]*\}/s,
  );
});
