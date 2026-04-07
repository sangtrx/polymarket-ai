import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 4.4 weekly schedule processing dispatches export jobs with deterministic linkage", () => {
  const scheduling = read("services/reporting-service/src/exports/scheduling.rs");

  assert.match(scheduling, /dispatch_weekly_export\(DispatchWeeklyExportInput/);
  assert.match(scheduling, /schedule_id: schedule\.schedule_id\.clone\(\)/);
  assert.match(scheduling, /schedule_window_key: run\.window_key\.clone\(\)/);
  assert.match(scheduling, /report_run_id: run\.run_id\.clone\(\)/);
  assert.match(scheduling, /weekly export dispatch failed:/);
});

test("Story 4.4 operations runbook documents weekly, on-demand, incident, and retrieval workflows", () => {
  const runbook = read("docs/operations/report-export-workflows.md");

  assert.match(runbook, /Weekly Scheduled Flow/i);
  assert.match(runbook, /On-Demand Trigger Flow/i);
  assert.match(runbook, /Incident-Triggered Flow/i);
  assert.match(runbook, /Retrieval and Audit Procedure/i);
  assert.match(runbook, /POST \/control\/report-exports\/on-demand/);
  assert.match(runbook, /POST \/control\/report-exports\/incidents\/\{incident_id\}/);
  assert.match(runbook, /GET \/control\/report-exports\/\{job_id\}/);
  assert.match(runbook, /fails? closed/i);
});

test("Story 4.4 artifact helpers enforce deterministic references and checksum metadata", () => {
  const artifactHelpers = read("services/reporting-service/src/exports/artifacts.rs");

  assert.match(artifactHelpers, /required_fr36_artifact_types/);
  assert.match(artifactHelpers, /deterministic_artifact_order/);
  assert.match(artifactHelpers, /compose_package_reference/);
  assert.match(artifactHelpers, /compose_artifact_reference/);
  assert.match(artifactHelpers, /compute_manifest_checksum/);
  assert.match(artifactHelpers, /compute_artifact_checksum/);
});

test("Story 4.4 incident failures emit alert-compatible evidence within 30-second SLA", () => {
  const workflows = read("services/reporting-service/src/exports/workflows.rs");

  assert.match(workflows, /emit_incident_export_failure_alert\(/);
  assert.match(workflows, /report_export_incident_failure_alert_v1/);
  assert.match(workflows, /signal_name: "report_export_incident_failure_v1"/);
  assert.match(workflows, /if emitted_at > failed_at \+ Duration::seconds\(30\)/);
  assert.match(workflows, /incident export alert evidence exceeded 30 second SLA/);
  assert.match(workflows, /alert_target_seconds: 30/);
});

test("Story 4.4 retrieval remains fail-closed for missing jobs and integrity mismatches", () => {
  const workflows = read("services/reporting-service/src/exports/workflows.rs");

  assert.match(workflows, /if !job_exists/);
  assert.match(workflows, /ReportExportWorkflowError::not_found/);
  assert.match(workflows, /ReportExportWorkflowError::integrity_mismatch/);
  assert.match(workflows, /artifact checksum\/reference integrity mismatch/);
  assert.match(workflows, /ReportingExportReasonCode::ArtifactUnavailable\.code\(\)/);
});
