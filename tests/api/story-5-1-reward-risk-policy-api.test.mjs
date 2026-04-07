import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 5.1 reward-risk API exposes authenticated upsert and read routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/reward-risk\/policies\/\{policy_key\}/);
  assert.match(routes, /post\(upsert_reward_risk_policy\)\.get\(read_reward_risk_policy\)/);
  assert.match(routes, /pub async fn upsert_reward_risk_policy/);
  assert.match(routes, /pub async fn read_reward_risk_policy/);
  assert.match(routes, /authorize_critical_action\(&state, &actor, &endpoint, "POST"\)/);
  assert.match(routes, /authorize_critical_action\(&state, &actor, &endpoint, "GET"\)/);
});

test("Story 5.1 reward-risk API maps deterministic machine-readable error status codes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /fn reward_risk_service_error_status\(code: &str\) -> StatusCode/);
  assert.match(
    routes,
    /RewardRiskReasonCode::InvalidPayload\.code\(\)[\s\S]*RewardRiskReasonCode::InvalidPolicyKey\.code\(\)[\s\S]*RewardRiskReasonCode::InvalidThreshold\.code\(\)[\s\S]*StatusCode::BAD_REQUEST/s,
  );
  assert.match(routes, /"reward_risk_unauthorized_role" => StatusCode::FORBIDDEN/);
  assert.match(routes, /"reward_risk_constraint_violation" => StatusCode::CONFLICT/);
  assert.match(
    routes,
    /RewardRiskReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*"reward_risk_query_failed"[\s\S]*"reward_risk_row_decode_failed"[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 5.1 reward-risk API emits unauthorized security signal evidence for denied mutations", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /fn reward_risk_service_error_response\(/);
  assert.match(routes, /error_code == "reward_risk_unauthorized_role"/);
  assert.match(routes, /name: "unauthorized_reward_risk_mutation_attempt_v1"/);
  assert.match(routes, /alert_compatible: true/);
  assert.match(routes, /alert_target_seconds: 30/);
  assert.match(routes, /axum::Json\(RewardRiskServiceErrorResponse \{/);
  assert.match(routes, /field_errors: field_errors/);
  assert.match(routes, /correlation_id: actor\.correlation_id\.clone\(\)/);
});

test("Story 5.1 reward-risk orchestrator keeps default-threshold fallback and privileged role boundaries", () => {
  const orchestrator = read("services/governance-service/src/reward_risk/mod.rs");

  assert.match(orchestrator, /min_reward_per_risk: REWARD_RISK_DEFAULT_THRESHOLD/);
  assert.match(orchestrator, /RewardRiskReasonCode::DefaultThresholdApplied/);
  assert.match(orchestrator, /default_threshold_applied: true/);
  assert.match(orchestrator, /"operational_control" \| "administrative_actions"/);
  assert.match(orchestrator, /RewardRiskServiceError::unauthorized_role\(\)/);
});
