import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 4.4 control-api exposes report export trigger and retrieval routes", () => {
  const routesSource = read("services/control-api/src/routes/mod.rs");

  assert.match(routesSource, /\/control\/report-exports\/on-demand/);
  assert.match(routesSource, /\/control\/report-exports\/incidents\/\{incident_id\}/);
  assert.match(routesSource, /\/control\/report-exports\/\{job_id\}/);
  assert.match(routesSource, /\/control\/report-exports\/\{job_id\}\/artifacts/);
  assert.match(
    routesSource,
    /\/control\/report-exports\/\{job_id\}\/artifacts\/\{artifact_id\}/,
  );
  assert.match(routesSource, /report_export_trigger_applied_v1/);
  assert.match(routesSource, /report_export_transition_rejected_v1/);
});

test("Story 4.4 migration introduces only export_jobs/export_artifacts with weekly idempotency", () => {
  const migration = read(
    "crates/persistence/migrations/20260407062000_export_jobs_export_artifacts.sql",
  );

  assert.match(migration, /CREATE TABLE IF NOT EXISTS export_jobs/);
  assert.match(migration, /CREATE TABLE IF NOT EXISTS export_artifacts/);
  assert.match(migration, /CREATE UNIQUE INDEX IF NOT EXISTS uq_export_jobs_weekly_idempotency/);
  assert.match(
    migration,
    /artifact_type IN \(\s*'promotion_decisions',\s*'validation_evidence',\s*'reconciliation_summary',\s*'access_audits',\s*'incident_postmortems'/s,
  );
});

test("Story 4.4 QA command is published in root package scripts", () => {
  const packageJson = JSON.parse(read("package.json"));
  assert.equal(typeof packageJson.scripts?.["qa:test:story-4-4"], "string");
  assert.match(packageJson.scripts["qa:test:story-4-4"], /reporting_export::tests::/);
  assert.match(packageJson.scripts["qa:test:story-4-4"], /postgres::export_jobs::tests::/);
  assert.match(packageJson.scripts["qa:test:story-4-4"], /routes::tests::report_export_/);
  assert.match(
    packageJson.scripts["qa:test:story-4-4"],
    /tests\/contract\/story-4-4\*\.test\.mjs/,
  );
  assert.match(packageJson.scripts["qa:test:story-4-4"], /tests\/api\/story-4-4\*\.test\.mjs/);
  assert.match(
    packageJson.scripts["qa:test:story-4-4"],
    /tests\/e2e\/story-4-4\*\.test\.mjs/,
  );
});
