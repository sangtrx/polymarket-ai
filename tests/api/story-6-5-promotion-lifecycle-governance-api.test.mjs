import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.5 exposes authenticated promotion-decision start/read/list routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/research\/promotion-decisions"/);
  assert.match(
    routes,
    /post\(start_promotion_decision\)\.get\(list_promotion_decisions\)/,
  );
  assert.match(
    routes,
    /\/control\/research\/promotion-decisions\/\{decision_id\}"/,
  );
  assert.match(routes, /get\(read_promotion_decision\)/);
});

test("Story 6.5 payload/query/response contracts follow canonical data/meta/error envelopes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct PromotionDecisionStartPayload \{[\s\S]*pub candidate_id: String,[\s\S]*pub validation_run_id: String,[\s\S]*pub lifecycle_action: String,[\s\S]*pub observed_metrics: serde_json::Value,[\s\S]*pub thresholds: Vec<domain::research::PromotionThresholdDefinition>,[\s\S]*pub evidence_packet: serde_json::Value,[\s\S]*pub shadow_readiness: Option<serde_json::Value>,[\s\S]*pub approval_request_id: Option<String>,[\s\S]*pub approval_reference: Option<String>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct PromotionDecisionsQuery \{[\s\S]*pub candidate_id: String,[\s\S]*pub limit: Option<i64>,[\s\S]*pub decided_after_utc: Option<String>,[\s\S]*pub decided_before_utc: Option<String>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct PromotionDecisionEnvelope<T: Serialize> \{[\s\S]*pub data: Option<T>,[\s\S]*pub meta: PromotionDecisionMeta,[\s\S]*pub error: Option<PromotionDecisionEnvelopeError>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub enum PromotionDecisionData \{[\s\S]*Decision[\s\S]*Decisions[\s\S]*\}/s,
  );
  assert.match(routes, /pub struct PromotionDecisionEnvelopeError \{/);
});

test("Story 6.5 status mapping is deterministic and fail-closed", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn promotion_decision_service_error_status\(code: &str\) -> StatusCode/,
  );
  assert.match(
    routes,
    /PromotionDecisionReasonCode::InvalidPayload\.code\(\)[\s\S]*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /PromotionDecisionReasonCode::UnauthorizedRole\.code\(\)[\s\S]*StatusCode::FORBIDDEN/s,
  );
  assert.match(
    routes,
    /PromotionDecisionReasonCode::DecisionNotFound\.code\(\)[\s\S]*PromotionDecisionReasonCode::MissingEvidence\.code\(\)[\s\S]*PromotionDecisionReasonCode::ThresholdFailed\.code\(\)[\s\S]*PromotionDecisionReasonCode::GateDenied\.code\(\)[\s\S]*PromotionDecisionReasonCode::ApprovalRequired\.code\(\)[\s\S]*PromotionDecisionReasonCode::ApprovalInvalidState\.code\(\)[\s\S]*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /PromotionDecisionReasonCode::DependencyUnavailable\.code\(\)[\s\S]*PromotionDecisionReasonCode::StateUnavailable\.code\(\)[\s\S]*PromotionDecisionReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 6.5 mutation path integrates governed sign-off for strategy_promotion_override", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /evaluate_execution\(EvaluateApprovalExecutionInput \{/);
  assert.match(routes, /action_id: "strategy_promotion_override"\.to_string\(\)/);
  assert.match(
    routes,
    /approval_reference cannot be supplied directly; provide approval_request_id and rely on approval workflow evidence\./,
  );
});

test("Story 6.5 control-api state/startup wiring includes promotion decision orchestration", () => {
  const middleware = read("services/control-api/src/middleware/mod.rs");
  const main = read("services/control-api/src/main.rs");

  assert.match(
    middleware,
    /pub research_promotion_decision_orchestrator: Arc<dyn PromotionDecisionOrchestrator>/,
  );
  assert.match(
    middleware,
    /with_research_promotion_decision_orchestrator\(/,
  );
  assert.match(main, /PromotionDecisionService::postgres\(/);
  assert.match(main, /with_research_promotion_decision_orchestrator\(/);
});

test("Story 6.5 research-gateway exports promotion decision orchestration seams", () => {
  const promotionMod = read("services/research-gateway/src/promotion/mod.rs");
  const decisions = read("services/research-gateway/src/promotion/decisions.rs");

  assert.match(promotionMod, /pub mod decisions;/);
  assert.match(decisions, /pub trait PromotionDecisionOrchestrator: Send \+ Sync/);
  assert.match(decisions, /fn start_promotion_decision\(/);
  assert.match(decisions, /fn read_promotion_decision\(/);
  assert.match(decisions, /fn list_promotion_decisions\(/);
  assert.match(decisions, /evaluate_promotion_entry_gates\(/);
});

test("Story 6.5 unauthorized promotion-decision denials emit machine-readable security signals", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /unauthorized_promotion_decision_read_attempt_v1/);
  assert.match(routes, /unauthorized_promotion_decision_mutation_attempt_v1/);
});

test("Story 6.5 list route canonicalizes candidate ids and propagates effective correlation ids", () => {
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
    /list_promotion_decisions\(ListPromotionDecisionsInput \{[\s\S]*candidate_id: candidate_id\.clone\(\),[\s\S]*correlation_id: effective_correlation_id\.clone\(\),[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /promotion_decision_list_response\([\s\S]*canonical_candidate_id,[\s\S]*effective_correlation_id,[\s\S]*authorization\.timestamp_utc,[\s\S]*\)/s,
  );
});

test("Story 6.5 error envelopes and deny audit records keep correlation continuity", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn promotion_decision_service_error_response\([\s\S]*correlation_id: String,[\s\S]*PrivilegedAuditRecord \{[\s\S]*correlation_id: correlation_id\.clone\(\),[\s\S]*\}\s*;[\s\S]*PromotionDecisionMeta \{[\s\S]*correlation_id,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /parameters: json!\(\{[\s\S]*"endpoint": endpoint\.clone\(\),[\s\S]*"http_method": http_method,[\s\S]*"error_code": error\.code,[\s\S]*\}\)/s,
  );
});
