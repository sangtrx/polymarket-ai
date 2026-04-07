import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.4 exposes authenticated shadow-evaluation start/read/list routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/research\/shadow-evaluations"/);
  assert.match(
    routes,
    /post\(start_shadow_evaluation\)\.get\(list_shadow_evaluations\)/,
  );
  assert.match(
    routes,
    /\/control\/research\/shadow-evaluations\/\{evaluation_id\}"/,
  );
  assert.match(routes, /get\(read_shadow_evaluation\)/);
});

test("Story 6.4 payload/query/response contracts follow canonical data/meta/error envelopes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct ShadowEvaluationStartPayload \{[\s\S]*pub candidate_id: String,[\s\S]*pub validation_run_id: String,[\s\S]*pub market_context: serde_json::Value,[\s\S]*pub signal_decisions: serde_json::Value,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ShadowEvaluationsQuery \{[\s\S]*pub candidate_id: String,[\s\S]*pub limit: Option<i64>,[\s\S]*pub started_after_utc: Option<String>,[\s\S]*pub started_before_utc: Option<String>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ShadowEvaluationEnvelope<T: Serialize> \{[\s\S]*pub data: Option<T>,[\s\S]*pub meta: ShadowEvaluationMeta,[\s\S]*pub error: Option<ShadowEvaluationEnvelopeError>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub enum ShadowEvaluationData \{[\s\S]*Evaluation[\s\S]*Evaluations[\s\S]*\}/s,
  );
});

test("Story 6.4 status mapping is deterministic and fail-closed", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn shadow_evaluation_service_error_status\(code: &str\) -> StatusCode/,
  );
  assert.match(
    routes,
    /ShadowEvaluationReasonCode::InvalidPayload\.code\(\)[\s\S]*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /ShadowEvaluationReasonCode::UnauthorizedRole\.code\(\)[\s\S]*StatusCode::FORBIDDEN/s,
  );
  assert.match(
    routes,
    /ShadowEvaluationReasonCode::ValidationRunIneligible\.code\(\)[\s\S]*ShadowEvaluationReasonCode::EvaluationNotFound\.code\(\)[\s\S]*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /ShadowEvaluationReasonCode::DependencyUnavailable\.code\(\)[\s\S]*ShadowEvaluationReasonCode::StateUnavailable\.code\(\)[\s\S]*ShadowEvaluationReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 6.4 control-api state/startup wiring includes shadow orchestration", () => {
  const middleware = read("services/control-api/src/middleware/mod.rs");
  const main = read("services/control-api/src/main.rs");

  assert.match(
    middleware,
    /pub research_shadow_mode_orchestrator: Arc<dyn ShadowModeOrchestrator>/,
  );
  assert.match(middleware, /with_research_shadow_mode_orchestrator\(/);
  assert.match(main, /ShadowModeService::postgres\(/);
  assert.match(main, /with_research_shadow_mode_orchestrator\(/);
});

test("Story 6.4 research-gateway exports shadow-mode orchestration seams", () => {
  const validationMod = read("services/research-gateway/src/validation/mod.rs");
  const shadowMode = read("services/research-gateway/src/validation/shadow_mode.rs");

  assert.match(validationMod, /pub mod shadow_mode;/);
  assert.match(shadowMode, /pub trait ShadowModeOrchestrator: Send \+ Sync/);
  assert.match(shadowMode, /fn start_shadow_evaluation\(/);
  assert.match(shadowMode, /fn read_shadow_evaluation\(/);
  assert.match(shadowMode, /fn list_shadow_evaluations\(/);
});

test("Story 6.4 unauthorized shadow-evaluation denials emit machine-readable security signals", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /unauthorized_shadow_evaluation_read_attempt_v1/);
  assert.match(routes, /unauthorized_shadow_evaluation_mutation_attempt_v1/);
});

test("Story 6.4 list route canonicalizes candidate ids and propagates effective correlation ids", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /let effective_correlation_id = query[\s\S]*unwrap_or_else\(\|\| actor\.correlation_id\.clone\(\)\);/s,
  );
  assert.match(
    routes,
    /let canonical_candidate_id = normalize_research_identifier\(&candidate_id\);/,
  );
  assert.match(
    routes,
    /list_shadow_evaluations\(ListShadowEvaluationsInput \{[\s\S]*candidate_id: candidate_id\.clone\(\),[\s\S]*correlation_id: effective_correlation_id\.clone\(\),[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /shadow_evaluation_list_response\([\s\S]*canonical_candidate_id,[\s\S]*effective_correlation_id,[\s\S]*authorization\.timestamp_utc,[\s\S]*\)/s,
  );
});

test("Story 6.4 error envelopes and deny audit records keep correlation continuity", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn shadow_evaluation_service_error_response\([\s\S]*correlation_id: String,[\s\S]*PrivilegedAuditRecord \{[\s\S]*correlation_id: correlation_id\.clone\(\),[\s\S]*\}\s*;[\s\S]*ShadowEvaluationMeta \{[\s\S]*correlation_id,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /parameters: json!\(\{[\s\S]*"endpoint": endpoint\.clone\(\),[\s\S]*"http_method": http_method,[\s\S]*"error_code": error\.code,[\s\S]*\}\)/s,
  );
});
