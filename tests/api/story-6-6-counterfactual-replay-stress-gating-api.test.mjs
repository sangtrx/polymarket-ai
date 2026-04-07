import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.6 exposes authenticated counterfactual replay start/read/list routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/research\/counterfactual-replay-runs"/);
  assert.match(
    routes,
    /post\(start_counterfactual_replay_run\)\.get\(list_counterfactual_replay_runs\)/,
  );
  assert.match(
    routes,
    /\/control\/research\/counterfactual-replay-runs\/\{replay_run_id\}"/,
  );
  assert.match(routes, /get\(read_counterfactual_replay_run\)/);
});

test("Story 6.6 replay payload/query/response contracts follow canonical envelopes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct CounterfactualReplayStartPayload \{[\s\S]*pub candidate_id: String,[\s\S]*pub validation_run_id: String,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct CounterfactualReplayRunsQuery \{[\s\S]*pub candidate_id: String,[\s\S]*pub limit: Option<i64>,[\s\S]*pub started_after_utc: Option<String>,[\s\S]*pub started_before_utc: Option<String>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct CounterfactualReplayEnvelope<T: Serialize> \{[\s\S]*pub data: Option<T>,[\s\S]*pub meta: CounterfactualReplayMeta,[\s\S]*pub error: Option<CounterfactualReplayEnvelopeError>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub enum CounterfactualReplayData \{[\s\S]*ReplayRun[\s\S]*ReplayRuns[\s\S]*\}/s,
  );
  assert.match(routes, /pub struct CounterfactualReplayEnvelopeError \{/);
});

test("Story 6.6 replay status mapping is deterministic and fail-closed", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn counterfactual_replay_service_error_status\(code: &str\) -> StatusCode/,
  );
  assert.match(
    routes,
    /CounterfactualReplayReasonCode::InvalidPayload\.code\(\)[\s\S]*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /CounterfactualReplayReasonCode::UnauthorizedRole\.code\(\)[\s\S]*StatusCode::FORBIDDEN/s,
  );
  assert.match(
    routes,
    /CounterfactualReplayReasonCode::RunNotFound\.code\(\)[\s\S]*CounterfactualReplayReasonCode::ScenarioIncomplete\.code\(\)[\s\S]*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /CounterfactualReplayReasonCode::DependencyUnavailable\.code\(\)[\s\S]*CounterfactualReplayReasonCode::StateUnavailable\.code\(\)[\s\S]*CounterfactualReplayReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 6.6 control-api state/startup wiring includes replay orchestration", () => {
  const middleware = read("services/control-api/src/middleware/mod.rs");
  const main = read("services/control-api/src/main.rs");

  assert.match(
    middleware,
    /pub research_counterfactual_replay_orchestrator: Arc<dyn CounterfactualReplayOrchestrator>/,
  );
  assert.match(
    middleware,
    /with_research_counterfactual_replay_orchestrator\(/,
  );
  assert.match(main, /CounterfactualReplayService::postgres\(/);
  assert.match(main, /with_research_counterfactual_replay_orchestrator\(/);
});

test("Story 6.6 research-gateway exports replay orchestration seams and promote integration", () => {
  const promotionMod = read("services/research-gateway/src/promotion/mod.rs");
  const replay = read("services/research-gateway/src/promotion/counterfactual_replay.rs");
  const decisions = read("services/research-gateway/src/promotion/decisions.rs");

  assert.match(promotionMod, /pub mod counterfactual_replay;/);
  assert.match(replay, /pub trait CounterfactualReplayOrchestrator: Send \+ Sync/);
  assert.match(replay, /fn start_counterfactual_replay\(/);
  assert.match(replay, /fn read_counterfactual_replay\(/);
  assert.match(replay, /fn list_counterfactual_replay_runs\(/);
  assert.match(decisions, /start_counterfactual_replay\(StartCounterfactualReplayInput \{/);
  assert.match(
    decisions,
    /PromotionDecisionReasonCode::ReplayGateDenied[\s\S]*\.code\(\)[\s\S]*\.to_string\(\)/s,
  );
});

test("Story 6.6 unauthorized replay denials emit machine-readable security signals", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /unauthorized_counterfactual_replay_read_attempt_v1/);
  assert.match(routes, /unauthorized_counterfactual_replay_mutation_attempt_v1/);
});

test("Story 6.6 replay list route canonicalizes candidate ids and keeps correlation continuity", () => {
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
    /list_counterfactual_replay_runs\(ListCounterfactualReplayRunsInput \{[\s\S]*candidate_id: candidate_id\.clone\(\),[\s\S]*correlation_id: effective_correlation_id\.clone\(\),[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /counterfactual_replay_list_response\([\s\S]*canonical_candidate_id,[\s\S]*effective_correlation_id,[\s\S]*authorization\.timestamp_utc,[\s\S]*\)/s,
  );
});

test("Story 6.6 route-level negative-path coverage includes replay envelope query/json/auth guards", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /counterfactual_replay_start_route_maps_json_rejection_to_bad_request_envelope/,
  );
  assert.match(
    routes,
    /counterfactual_replay_list_route_maps_query_rejection_to_bad_request_envelope/,
  );
  assert.match(
    routes,
    /counterfactual_replay_start_route_auth_denial_returns_replay_envelope/,
  );
  assert.match(
    routes,
    /counterfactual-replay-runs\?candidate_id=candidate::alpha-1&limit=abc/,
  );
});
