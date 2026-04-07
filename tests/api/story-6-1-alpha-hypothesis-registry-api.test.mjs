import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.1 exposes authenticated alpha-hypothesis register/read routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/research\/alpha-hypotheses\/\{hypothesis_id\}/);
  assert.match(routes, /post\(register_alpha_hypothesis\)\.get\(read_alpha_hypothesis\)/);
  assert.match(routes, /pub async fn register_alpha_hypothesis/);
  assert.match(routes, /pub async fn read_alpha_hypothesis/);
  assert.match(routes, /authorize_critical_action\(&state, &actor, &endpoint, "POST"\)/);
  assert.match(routes, /authorize_alpha_hypothesis_read\(&state, &actor, &endpoint\)/);
});

test("Story 6.1 API payload and evidence contracts enforce required FR6 metadata fields", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct AlphaHypothesisRegistrationPayload \{[\s\S]*pub feature_set_version: String,[\s\S]*pub target_regime: String,[\s\S]*pub expected_edge_source: String,[\s\S]*pub training_window_start_utc: String,[\s\S]*pub training_window_end_utc: String,[\s\S]*pub risk_assumptions: serde_json::Value,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct AlphaHypothesisDecisionResponse \{[\s\S]*pub hypothesis_id: String,[\s\S]*pub feature_set_version: String,[\s\S]*pub target_regime: String,[\s\S]*pub expected_edge_source: String,[\s\S]*pub training_window_start_utc: String,[\s\S]*pub training_window_end_utc: String,[\s\S]*pub risk_assumptions: serde_json::Value,[\s\S]*pub reason_code: String,[\s\S]*\}/s,
  );
});

test("Story 6.1 machine-readable status mapping keeps deterministic fail-closed outcomes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /fn alpha_hypothesis_service_error_status\(code: &str\) -> StatusCode/);
  assert.match(
    routes,
    /AlphaHypothesisReasonCode::InvalidPayload\.code\(\)[\s\S]*AlphaHypothesisReasonCode::InvalidTrainingWindow\.code\(\)[\s\S]*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /AlphaHypothesisReasonCode::UnauthorizedRole\.code\(\)\s*=>\s*StatusCode::FORBIDDEN/s,
  );
  assert.match(routes, /"alpha_hypothesis_constraint_violation" => StatusCode::CONFLICT/);
  assert.match(
    routes,
    /AlphaHypothesisReasonCode::DatasetSnapshotUnresolved\.code\(\)[\s\S]*AlphaHypothesisReasonCode::NotFound\.code\(\)[\s\S]*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /AlphaHypothesisReasonCode::DatasetSnapshotUnavailable\.code\(\)[\s\S]*AlphaHypothesisReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 6.1 control-api wiring includes research orchestrator in state and startup", () => {
  const middleware = read("services/control-api/src/middleware/mod.rs");
  const main = read("services/control-api/src/main.rs");

  assert.match(middleware, /pub research_hypothesis_orchestrator: Arc<dyn HypothesisRegistryOrchestrator>/);
  assert.match(middleware, /with_research_hypothesis_orchestrator\(/);
  assert.match(main, /HypothesisRegistryService::postgres\(\s*pool\.clone\(\),?\s*\)/);
});

test("Story 6.1 research-gateway defines dataset snapshot registry seam and fail-closed reason codes", () => {
  const orchestrator = read("services/research-gateway/src/validation/hypothesis_registry.rs");

  assert.match(orchestrator, /pub trait DatasetSnapshotRegistryPort: Send \+ Sync/);
  assert.match(orchestrator, /fn is_registered\(\s*&self,\s*feature_set_version: &str,\s*\)/);
  assert.match(
    orchestrator,
    /AlphaHypothesisReasonCode::DatasetSnapshotUnresolved\.code\(\)/,
  );
  assert.match(
    orchestrator,
    /AlphaHypothesisReasonCode::DatasetSnapshotUnavailable\.code\(\)/,
  );
  assert.match(
    orchestrator,
    /"operational_control" \| "administrative_actions"/,
  );
});
