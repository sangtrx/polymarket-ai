import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 4.3 schedule API exposes authenticated mutation and run-history endpoints", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/report-schedules\/\{schedule_id\}/);
  assert.match(routes, /\/control\/report-schedules\/\{schedule_id\}\/pause/);
  assert.match(routes, /\/control\/report-schedules\/\{schedule_id\}\/resume/);
  assert.match(routes, /\/control\/report-schedules\/\{schedule_id\}\/runs/);

  assert.match(routes, /pub async fn upsert_report_schedule/);
  assert.match(routes, /pub async fn pause_report_schedule/);
  assert.match(routes, /pub async fn resume_report_schedule/);
  assert.match(routes, /pub async fn query_report_schedule_runs/);

  assert.match(routes, /authorize_critical_action\(&state, &actor, &endpoint, "POST"\)/);
  assert.match(routes, /authorize_report_schedule_read\(&state, &actor, &endpoint\)/);
});

test("Story 4.3 schedule API keeps deterministic machine-readable error/status mapping", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /value if value == ReportingScheduleReasonCode::InvalidPayload\.code\(\)\s*=>\s*\{\s*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /value if value == ReportingScheduleReasonCode::Unauthorized\.code\(\) => StatusCode::FORBIDDEN/,
  );
  assert.match(
    routes,
    /value if value == ReportingScheduleReasonCode::NotFound\.code\(\) => StatusCode::NOT_FOUND/,
  );
  assert.match(
    routes,
    /value if value == ReportingScheduleReasonCode::StaleEvidence\.code\(\) => StatusCode::CONFLICT/,
  );
  assert.match(routes, /"report_schedule_constraint_violation" => StatusCode::CONFLICT/);
  assert.match(routes, /ReportingScheduleReasonCode::DependencyUnavailable\.code\(\)/);
  assert.match(routes, /ReportingScheduleReasonCode::PersistenceUnavailable\.code\(\)/);
  assert.match(routes, /ReportingScheduleReasonCode::AlertUnavailable\.code\(\)/);
  assert.match(routes, /"report_schedule_query_failed"/);
  assert.match(routes, /"report_schedule_row_decode_failed"/);
  assert.match(routes, /StatusCode::SERVICE_UNAVAILABLE/);
});

test("Story 4.3 schedule API emits transition telemetry and security-signal evidence", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /signal_name: "report_schedule_transition_applied_v1"/);
  assert.match(routes, /signal_name: "report_schedule_transition_rejected_v1"/);
  assert.match(routes, /name: "unauthorized_report_schedule_mutation_attempt_v1"/);
  assert.match(routes, /ReportScheduleMutationResponse/);
  assert.match(routes, /ReportScheduleServiceErrorResponse/);
  assert.match(routes, /run_count/);
  assert.match(routes, /source_context/);
  assert.match(routes, /impacted_system/);
});
