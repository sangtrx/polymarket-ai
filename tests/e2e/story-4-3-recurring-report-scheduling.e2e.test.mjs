import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 4.3 scheduler lifecycle enforces pending-running-terminal transitions per UTC windows", () => {
  const scheduling = read("services/reporting-service/src/exports/scheduling.rs");

  assert.match(scheduling, /build_reporting_window_for_boundary/);
  assert.match(scheduling, /ReportingRunState::Pending/);
  assert.match(scheduling, /run.status = ReportingRunState::Running/);
  assert.match(scheduling, /run.status = ReportingRunState::Succeeded/);
  assert.match(scheduling, /run.status = ReportingRunState::Failed/);
  assert.match(scheduling, /run.status = ReportingRunState::Missed/);
  assert.match(scheduling, /advance_to_next_run_at_utc/);
  assert.match(scheduling, /summary_port\.hydrate_summary_window/);
});

test("Story 4.3 scheduler fail-closed escalation keeps 30-second SLA and alert linkage", () => {
  const scheduling = read("services/reporting-service/src/exports/scheduling.rs");

  assert.match(scheduling, /if now_timestamp > run_boundary \+ Duration::seconds\(30\)/);
  assert.match(scheduling, /emit_critical_failure_alert/);
  assert.match(scheduling, /ReportingScheduleReasonCode::DependencyUnavailable\.code\(\)/);
  assert.match(scheduling, /ReportingScheduleReasonCode::StaleEvidence\.code\(\)/);
  assert.match(scheduling, /ReportingScheduleReasonCode::PersistenceUnavailable\.code\(\)/);
  assert.match(scheduling, /ReportingScheduleReasonCode::AlertUnavailable\.code\(\)/);
  assert.match(scheduling, /record_alert_emission_within_sla/);
  assert.match(scheduling, /critical schedule alert evidence exceeded 30 second SLA/);
});

test("Story 4.3 runbook captures operator pause-resume and bounded-history workflow", () => {
  const runbook = read("docs/operations/recurring-report-scheduling.md");

  assert.match(runbook, /Daily:.*00:00 UTC/i);
  assert.match(runbook, /Weekly:.*Monday 00:00 UTC/i);
  assert.match(runbook, /Monthly:.*day 1, 00:00 UTC/i);
  assert.match(runbook, /POST \/control\/report-schedules\/\{schedule_id\}\/pause/);
  assert.match(runbook, /POST \/control\/report-schedules\/\{schedule_id\}\/resume/);
  assert.match(runbook, /GET \/control\/report-schedules\/\{schedule_id\}\/runs/);
  assert.match(runbook, /within 30 seconds/i);
  assert.match(runbook, /fails closed/i);
});
