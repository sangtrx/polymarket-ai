import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 4.3 control-api exposes report schedule mutation and history routes", () => {
  const routesSource = read("services/control-api/src/routes/mod.rs");

  assert.match(routesSource, /\/control\/report-schedules\/\{schedule_id\}/);
  assert.match(routesSource, /\/control\/report-schedules\/\{schedule_id\}\/pause/);
  assert.match(routesSource, /\/control\/report-schedules\/\{schedule_id\}\/resume/);
  assert.match(routesSource, /\/control\/report-schedules\/\{schedule_id\}\/runs/);
  assert.match(routesSource, /report_schedule_transition_applied_v1/);
  assert.match(routesSource, /report_schedule_transition_rejected_v1/);
});

test("Story 4.3 scheduler enforces NFR15 critical failure escalation and missed-run boundary", () => {
  const schedulerSource = read("services/reporting-service/src/exports/scheduling.rs");

  assert.match(schedulerSource, /Duration::seconds\(30\)/);
  assert.match(schedulerSource, /ReportingScheduleReasonCode::RunMissed/);
  assert.match(schedulerSource, /emit_critical_failure_alert/);
  assert.match(schedulerSource, /reporting-service scheduler/);
});

test("Story 4.3 runbook documents cadence, pause\/resume, and escalation path", () => {
  const runbook = read("docs/operations/recurring-report-scheduling.md");

  assert.match(runbook, /daily.*00:00 UTC/i);
  assert.match(runbook, /weekly.*Monday 00:00 UTC/i);
  assert.match(runbook, /monthly.*day 1, 00:00 UTC/i);
  assert.match(runbook, /pause/i);
  assert.match(runbook, /resume/i);
  assert.match(runbook, /critical/i);
  assert.match(runbook, /within 30 seconds/i);
});

test("Story 4.3 QA command is published in root package scripts", () => {
  const packageJson = JSON.parse(read("package.json"));
  assert.equal(typeof packageJson.scripts?.["qa:test:story-4-3"], "string");
  assert.match(packageJson.scripts["qa:test:story-4-3"], /cargo test -p domain/);
  assert.match(packageJson.scripts["qa:test:story-4-3"], /tests\/contract\/story-4-3/);
  assert.match(packageJson.scripts["qa:test:story-4-3"], /tests\/api\/story-4-3\*\.test\.mjs/);
  assert.match(packageJson.scripts["qa:test:story-4-3"], /tests\/e2e\/story-4-3\*\.test\.mjs/);
});
