import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.2 exposes authenticated validation-gate policy mutate/read/evaluate routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /\/control\/research\/validation-gate-policies\/\{policy_key\}/,
  );
  assert.match(
    routes,
    /post\(upsert_validation_gate_policy\)\.get\(read_validation_gate_policy\)/,
  );
  assert.match(
    routes,
    /\/control\/research\/validation-gate-policies\/evaluate\/\{stage\}/,
  );
  assert.match(routes, /post\(evaluate_validation_gates\)/);
  assert.match(routes, /pub async fn list_validation_gate_policies/);
});

test("Story 6.2 payloads and responses use canonical data/meta/error envelope contracts", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct ValidationGatePolicyPayload \{[\s\S]*pub gate_type: String,[\s\S]*pub stage_scope: String,[\s\S]*pub metric_key: String,[\s\S]*pub comparator: String,[\s\S]*pub threshold_value: f64,[\s\S]*pub mandatory: bool,[\s\S]*pub diagnostics: serde_json::Value,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ValidationGateEnvelope<T: Serialize> \{[\s\S]*pub data: Option<T>,[\s\S]*pub meta: ValidationGateMeta,[\s\S]*pub error: Option<ValidationGateEnvelopeError>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub enum ValidationGateData \{[\s\S]*Policy[\s\S]*Policies[\s\S]*Evaluation[\s\S]*\}/s,
  );
});

test("Story 6.2 machine-readable status mapping is deterministic and fail-closed", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn validation_gate_service_error_status\(code: &str\) -> StatusCode/,
  );
  assert.match(
    routes,
    /ValidationGateReasonCode::InvalidPayload\.code\(\)\s*=>\s*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /ValidationGateReasonCode::UnauthorizedRole\.code\(\)\s*=>\s*StatusCode::FORBIDDEN/s,
  );
  assert.match(
    routes,
    /"validation_gate_policy_constraint_violation"\s*=>\s*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /ValidationGateReasonCode::PolicyNotFound\.code\(\)[\s\S]*ValidationGateReasonCode::PolicyUnresolved\.code\(\)[\s\S]*ValidationGateReasonCode::MissingMandatoryPolicy\.code\(\)[\s\S]*ValidationGateReasonCode::GateFailed\.code\(\)[\s\S]*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /ValidationGateReasonCode::DependencyUnavailable\.code\(\)[\s\S]*ValidationGateReasonCode::StateUnavailable\.code\(\)[\s\S]*ValidationGateReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 6.2 control-api state/startup wiring includes validation-gate orchestrator", () => {
  const middleware = read("services/control-api/src/middleware/mod.rs");
  const main = read("services/control-api/src/main.rs");

  assert.match(
    middleware,
    /pub research_validation_gate_orchestrator: Arc<dyn ValidationGatePolicyOrchestrator>/,
  );
  assert.match(middleware, /with_research_validation_gate_orchestrator\(/);
  assert.match(
    main,
    /ValidationGatePolicyService::postgres\(\s*pool\.clone\(\),?\s*\)/,
  );
});

test("Story 6.2 research-gateway exports gate-policy orchestration and training/promotion seams", () => {
  const validationMod = read("services/research-gateway/src/validation/mod.rs");
  const promotionMod = read("services/research-gateway/src/promotion/mod.rs");
  const gatePolicies = read(
    "services/research-gateway/src/validation/gate_policies.rs",
  );

  assert.match(validationMod, /pub mod gate_policies;/);
  assert.match(validationMod, /pub fn evaluate_training_entry_gates/);
  assert.match(promotionMod, /pub fn evaluate_promotion_entry_gates/);
  assert.match(
    gatePolicies,
    /pub trait ValidationGatePolicyOrchestrator: Send \+ Sync/,
  );
  assert.match(gatePolicies, /fn evaluate_validation_gates\(/);
});

test("Story 6.2 evaluation envelopes expose deterministic failed-gate diagnostics contracts", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /\/control\/research\/validation-gate-policies",\s*get\(list_validation_gate_policies\)/,
  );
  assert.match(
    routes,
    /pub struct ValidationGatePoliciesQuery \{[\s\S]*pub stage: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ValidationGateEvaluationItem \{[\s\S]*pub outcome: String,[\s\S]*pub reason_code: String,[\s\S]*pub failed_gate_ids: Vec<String>,[\s\S]*pub gate_results: Vec<ValidationGateEvaluationResultItem>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ValidationGateEvaluationResultItem \{[\s\S]*pub comparator: String,[\s\S]*pub threshold_value: f64,[\s\S]*pub observed_value: Option<f64>,[\s\S]*pub passed: bool,[\s\S]*pub reason_code: String,[\s\S]*\}/s,
  );
  assert.match(routes, /"failed_gate_ids": error\.failed_gate_ids/);
});

test("Story 6.2 unauthorized validation-gate denials emit machine-readable security signals", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /unauthorized_validation_gate_read_attempt_v1/);
  assert.match(routes, /unauthorized_validation_gate_mutation_attempt_v1/);
});
