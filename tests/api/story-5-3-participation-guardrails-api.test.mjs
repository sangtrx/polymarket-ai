import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 5.3 exposes authenticated participation-guardrail evidence query route", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/incidents\/participation-guardrails/);
  assert.match(routes, /get\(list_participation_guardrail_events\)/);
  assert.match(routes, /pub async fn list_participation_guardrail_events/);
  assert.match(routes, /"participation_guardrail_events_query"/);
});

test("Story 5.3 participation query contract carries deterministic filters and FR41 evidence fields", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct ParticipationGuardrailEventsQuery \{[\s\S]*pub market_id: Option<String>,[\s\S]*pub reason_code: Option<String>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*pub start_ts: Option<String>,[\s\S]*pub end_ts: Option<String>,[\s\S]*pub limit: Option<i64>,/s,
  );
  assert.match(
    routes,
    /pub struct ParticipationGuardrailEventItem \{[\s\S]*pub guardrail_mode: String,[\s\S]*pub reason_code: String,[\s\S]*pub market_id: String,[\s\S]*pub cluster_id: String,[\s\S]*pub correlation_id: String,[\s\S]*pub observed_at_utc: String,[\s\S]*pub threshold_liquidity_depth_usd: f64,[\s\S]*pub threshold_inactivity_pause_seconds: f64,[\s\S]*pub threshold_overnight_gap_seconds: f64,[\s\S]*pub normal_max_order_size_units: Option<f64>,[\s\S]*pub capped_max_order_size_units: Option<f64>,/s,
  );
  assert.match(
    routes,
    /load_participation_guardrail_events\([\s\S]*query\.market_id\.as_deref\(\),[\s\S]*query\.reason_code\.as_deref\(\),[\s\S]*query\.correlation_id\.as_deref\(\),[\s\S]*query\.start_ts\.as_deref\(\),[\s\S]*query\.end_ts\.as_deref\(\),[\s\S]*limit,[\s\S]*\)/s,
  );
});

test("Story 5.3 machine-readable status mapping includes FR41 persistence and decode failures", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /ParticipationGuardrailReasonCode::InvalidPayload\.code\(\)\s*=>\s*\{\s*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /ParticipationGuardrailReasonCode::DependencyUnavailable\.code\(\)[\s\S]*ParticipationGuardrailReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*"participation_guardrail_query_failed"[\s\S]*"participation_guardrail_row_decode_failed"[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(
    routes,
    /"participation_guardrail_constraint_violation"\s*=>\s*StatusCode::CONFLICT/,
  );
  assert.match(
    routes,
    /code if code == AlertReasonCode::Unauthorized\.code\(\)\s*=>\s*StatusCode::FORBIDDEN/,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 5.3 participation query response keeps deterministic accepted envelope semantics", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /fn participation_guardrail_query_response\(/);
  assert.match(
    routes,
    /let data_state = if events\.is_empty\(\) \{ "empty" \} else \{ "ready" \};/,
  );
  assert.match(
    routes,
    /let reason_code = if events\.is_empty\(\) \{\s*AlertReasonCode::NoTrigger\.code\(\)\.to_string\(\)\s*\} else \{\s*AlertReasonCode::Ready\.code\(\)\.to_string\(\)\s*\};/s,
  );
  assert.match(
    routes,
    /StatusCode::OK,[\s\S]*status: "accepted",[\s\S]*action: "participation_guardrail_events_query"/s,
  );
  assert.match(
    routes,
    /No FR41 participation guardrail events matched this query\. Continue monitoring liquidity and inactivity windows\./,
  );
  assert.match(
    routes,
    /Review FR41 guardrail modes and reason codes, then confirm operational thresholds and execution-size controls\./,
  );
});
