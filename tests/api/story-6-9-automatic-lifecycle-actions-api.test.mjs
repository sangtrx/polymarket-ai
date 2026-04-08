import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.9 exposes authenticated alpha-lifecycle-action routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/research\/alpha-lifecycle-actions"/);
  assert.match(
    routes,
    /post\(start_alpha_lifecycle_action\)\.get\(list_alpha_lifecycle_actions\)/,
  );
  assert.match(
    routes,
    /\/control\/research\/alpha-lifecycle-actions\/\{action_id\}"/,
  );
  assert.match(routes, /get\(read_alpha_lifecycle_action\)/);
});

test("Story 6.9 payload/query/envelope contracts follow canonical data/meta/error structure", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct AlphaLifecycleActionStartPayload \{[\s\S]*pub alpha_id: String,[\s\S]*pub action_type: String,[\s\S]*pub deallocation_policies: Vec<domain::research::AlphaLifecycleDeallocationPolicy>,[\s\S]*pub trade_count_30d: Option<i64>,[\s\S]*pub out_of_sample_sharpe_30d: Option<f64>,[\s\S]*pub promotion_failure_rate_last_10: Option<f64>,[\s\S]*pub approval_request_id: Option<String>,[\s\S]*pub approval_reference: Option<String>,[\s\S]*pub history_limit: Option<i64>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct AlphaLifecycleActionsQuery \{[\s\S]*pub alpha_id: String,[\s\S]*pub limit: Option<i64>,[\s\S]*pub acted_after_utc: Option<String>,[\s\S]*pub acted_before_utc: Option<String>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct AlphaLifecycleActionEnvelope<T: Serialize> \{[\s\S]*pub data: Option<T>,[\s\S]*pub meta: AlphaLifecycleActionMeta,[\s\S]*pub error: Option<AlphaLifecycleActionEnvelopeError>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub enum AlphaLifecycleActionData \{[\s\S]*Action[\s\S]*Actions[\s\S]*\}/s,
  );
});

test("Story 6.9 status mapping is deterministic and fail-closed", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn alpha_lifecycle_action_service_error_status\(code: &str\) -> StatusCode/,
  );
  assert.match(
    routes,
    /AlphaLifecycleReasonCode::InvalidPayload\.code\(\)[\s\S]*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /AlphaLifecycleReasonCode::UnauthorizedRole\.code\(\)[\s\S]*StatusCode::FORBIDDEN/s,
  );
  assert.match(
    routes,
    /"alpha_lifecycle_action_constraint_violation"[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(
    routes,
    /AlphaLifecycleReasonCode::DependencyUnavailable\.code\(\)[\s\S]*AlphaLifecycleReasonCode::StateUnavailable\.code\(\)[\s\S]*AlphaLifecycleReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 6.9 control-api state/startup wiring includes lifecycle-action orchestration", () => {
  const middleware = read("services/control-api/src/middleware/mod.rs");
  const main = read("services/control-api/src/main.rs");

  assert.match(
    middleware,
    /pub research_alpha_lifecycle_action_orchestrator: Arc<dyn AlphaLifecycleActionOrchestrator>/,
  );
  assert.match(
    middleware,
    /with_research_alpha_lifecycle_action_orchestrator\(/,
  );
  assert.match(main, /AlphaLifecycleActionService::postgres\(/);
  assert.match(main, /with_research_alpha_lifecycle_action_orchestrator\(/);
});

test("Story 6.9 research-gateway exports lifecycle-action orchestration seams", () => {
  const promotionMod = read("services/research-gateway/src/promotion/mod.rs");
  const lifecycle = read(
    "services/research-gateway/src/promotion/lifecycle_actions.rs",
  );

  assert.match(promotionMod, /pub mod lifecycle_actions;/);
  assert.match(
    lifecycle,
    /pub trait AlphaLifecycleActionOrchestrator: Send \+ Sync/,
  );
  assert.match(lifecycle, /fn start_alpha_lifecycle_action\(/);
  assert.match(lifecycle, /fn read_alpha_lifecycle_action\(/);
  assert.match(lifecycle, /fn list_alpha_lifecycle_actions\(/);
});

test("Story 6.9 readiness consumer reuses canonical lifecycle-action read seam for deallocation reflection", () => {
  const readiness = read("apps/operator-console/src/lib/governance/readiness.ts");

  assert.match(readiness, /\/control\/research\/alpha-lifecycle-actions/);
  assert.match(readiness, /parseAlphaLifecycleActionsData/);
  assert.match(readiness, /selectLatestLifecycleAction/);
  assert.match(
    readiness,
    /actionType === "deallocate" \|\| actionType === "stop_research"/,
  );
  assert.match(readiness, /return "deallocated";/);
});

test("Story 6.9 unauthorized denials emit machine-readable lifecycle security signals", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /unauthorized_alpha_lifecycle_action_read_attempt_v1/,
  );
  assert.match(
    routes,
    /unauthorized_alpha_lifecycle_action_mutation_attempt_v1/,
  );
});

test("Story 6.9 route-level test coverage includes success and fail-closed lifecycle paths", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /alpha_lifecycle_action_start_route_returns_data_meta_error_envelope/,
  );
  assert.match(
    routes,
    /alpha_lifecycle_action_read_list_routes_return_envelope_shapes/,
  );
  assert.match(
    routes,
    /alpha_lifecycle_action_start_route_maps_json_rejection_to_bad_request_envelope/,
  );
  assert.match(
    routes,
    /alpha_lifecycle_action_service_errors_emit_unauthorized_security_signals/,
  );
  assert.match(
    routes,
    /alpha_lifecycle_action_dependency_unavailable_maps_to_service_unavailable/,
  );
});

test("Story 6.9 read-path regressions keep conflict mapping and allow-audit continuity", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /alpha_lifecycle_action_read_not_found_maps_to_conflict/,
  );
  assert.match(
    routes,
    /AlphaLifecycleReasonCode::ActionNotFound\.code\(\)[\s\S]*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /alpha_lifecycle_action_read_audit_outcome_is_allow_for_successful_reads/,
  );
});

test("Story 6.9 QA command wiring executes Rust and story-scoped API/E2E checks", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-6-9"/);
  assert.match(
    packageJson,
    /cargo test -p domain research::tests::alpha_lifecycle_/,
  );
  assert.match(
    packageJson,
    /cargo test -p persistence postgres::alpha_lifecycle_actions::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p research-gateway promotion::lifecycle_actions::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p control-api routes::tests::alpha_lifecycle_action_/,
  );
  assert.match(packageJson, /tests\/api\/story-6-9\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-6-9\*\.test\.mjs/);
});
