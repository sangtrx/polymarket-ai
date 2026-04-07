import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 5.2 exposes authenticated regime-shift query and dispatch routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/incidents\/regime-shifts/);
  assert.match(routes, /get\(list_regime_shift_alerts\)/);
  assert.match(routes, /post\(dispatch_regime_shift_alerts\)/);
  assert.match(routes, /pub async fn list_regime_shift_alerts/);
  assert.match(routes, /pub async fn dispatch_regime_shift_alerts/);
  assert.match(routes, /"regime_shift_alerts_query"/);
  assert.match(routes, /"regime_shift_alert_dispatch"/);
});

test("Story 5.2 keeps actionability fields in dispatch input and alert evidence output contracts", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct RegimeShiftAlertDispatchPayload \{[\s\S]*pub recommended_next_action: Option<String>,[\s\S]*pub evidence_link: Option<String>,/s,
  );
  assert.match(
    routes,
    /pub struct RegimeShiftAlertItem \{[\s\S]*pub market_id: String,[\s\S]*pub cluster_id: String,[\s\S]*pub reason_code: String,[\s\S]*pub severity: String,[\s\S]*pub correlation_id: String,[\s\S]*pub observed_at: String,[\s\S]*pub recommended_next_action: String,[\s\S]*pub evidence_link: String,/s,
  );
});

test("Story 5.2 dispatch path reuses incident-alert dedupe and fallback delivery seams", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /let dedupe_window_seconds = payload\.dedupe_window_seconds\.unwrap_or\(300\)/);
  assert.match(routes, /should_emit_alert\(/);
  assert.match(routes, /simulate_alert_dispatch\(&alert, &simulation_payload\)/);
  assert.match(routes, /let mut transaction = match pool\.begin\(\)\.await/);
  assert.match(routes, /append_alert_delivery_attempt\(&mut \*transaction, attempt\)\.await/);
  assert.match(routes, /update_incident_alert_status\(/);
  assert.match(routes, /create_regime_shift_alert\(&mut \*transaction, &record\)\.await/);
  assert.match(routes, /transaction\.commit\(\)\.await/);
});

test("Story 5.2 machine-readable status mapping includes FR40 and persistence errors", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /RegimeShiftReasonCode::InvalidPayload\.code\(\) => StatusCode::BAD_REQUEST/);
  assert.match(
    routes,
    /RegimeShiftReasonCode::DependencyUnavailable\.code\(\)[\s\S]*RegimeShiftReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*"regime_shift_query_failed"[\s\S]*"regime_shift_row_decode_failed"[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /"regime_shift_constraint_violation" => StatusCode::CONFLICT/);
});
