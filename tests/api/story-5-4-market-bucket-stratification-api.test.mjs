import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 5.4 exposes authenticated market-bucket mutation and read routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/market-policy\/buckets\/\{market_id\}\/\{cluster_id\}/);
  assert.match(routes, /post\(update_market_bucket_profile\)\.get\(read_market_bucket_profile\)/);
  assert.match(routes, /pub async fn update_market_bucket_profile/);
  assert.match(routes, /pub async fn read_market_bucket_profile/);
  assert.match(routes, /authorize_critical_action\(&state, &actor, &endpoint, "POST"\)/);
  assert.match(routes, /authorize_critical_action\(&state, &actor, &endpoint, "GET"\)/);
});

test("Story 5.4 market-bucket API contracts carry canonical bucket and policy-link fields", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct MarketBucketProfilePayload \{[\s\S]*pub bucket_type: String,[\s\S]*pub risk_policy_key: String,[\s\S]*pub allocation_policy_key: String,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct MarketBucketProfileDecisionResponse \{[\s\S]*pub profile_id: String,[\s\S]*pub market_id: String,[\s\S]*pub cluster_id: String,[\s\S]*pub bucket_type: String,[\s\S]*pub risk_policy_key: String,[\s\S]*pub allocation_policy_key: String,[\s\S]*pub reason_code: String,[\s\S]*\}/s,
  );
  assert.match(routes, /"market_bucket_profile_update"/);
  assert.match(routes, /"market_bucket_profile_read"/);
});

test("Story 5.4 machine-readable status mapping preserves deterministic bucket error handling", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /MarketBucketReasonCode::InvalidPayload\.code\(\)[\s\S]*MarketBucketReasonCode::UnsupportedBucketType\.code\(\)[\s\S]*StatusCode::BAD_REQUEST/s,
  );
  assert.match(routes, /"market_policy_unauthorized_role" => StatusCode::FORBIDDEN/);
  assert.match(routes, /"market_bucket_constraint_violation" => \{\s*StatusCode::CONFLICT/s);
  assert.match(
    routes,
    /MarketBucketReasonCode::MappingUnavailable\.code\(\)\s*=>\s*\{\s*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(
    routes,
    /MarketBucketReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*"market_bucket_query_failed"[\s\S]*"market_bucket_row_decode_failed"[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
});

test("Story 5.4 update route forwards scoped identifiers and linked policy keys in one deterministic input contract", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /upsert_market_bucket_profile\(UpsertMarketBucketProfileInput \{[\s\S]*market_id,[\s\S]*cluster_id,[\s\S]*bucket_type: payload\.bucket_type,[\s\S]*risk_policy_key: payload\.risk_policy_key,[\s\S]*allocation_policy_key: payload\.allocation_policy_key,[\s\S]*\}\)/s,
  );
});

test("Story 5.4 market-bucket status mapping retains explicit internal-server fallback for unknown reason codes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn market_policy_service_error_status\(code: &str\) -> StatusCode \{[\s\S]*_ => StatusCode::INTERNAL_SERVER_ERROR,[\s\S]*\}/s,
  );
});

test("Story 5.4 governance orchestration emits FR42 evidence with normalized identifiers", () => {
  const orchestrator = read("services/governance-service/src/market_policy/mod.rs");

  assert.match(orchestrator, /canonical_market_bucket_profile_id\(/);
  assert.match(orchestrator, /canonicalize_market_bucket_profile\(&profile\)/);
  assert.match(orchestrator, /MarketBucketReasonCode::ProfileUpdated/);
  assert.match(orchestrator, /MarketBucketReasonCode::ProfileRead/);
  assert.match(orchestrator, /"operational_control" \| "administrative_actions"/);
});
