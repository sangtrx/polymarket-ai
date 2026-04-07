import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 4.4 export workflow service supports trigger, dispatch, and retrieval orchestration", () => {
  const workflows = read("services/reporting-service/src/exports/workflows.rs");

  assert.match(workflows, /fn trigger_on_demand_export\(/);
  assert.match(workflows, /fn trigger_incident_export\(/);
  assert.match(workflows, /fn dispatch_weekly_export\(/);
  assert.match(workflows, /fn query_export_job\(/);
  assert.match(workflows, /fn list_export_artifacts\(/);
  assert.match(workflows, /fn get_export_artifact\(/);
  assert.match(workflows, /validate_export_job_transition/);
  assert.match(workflows, /required_fr36_artifact_types/);
  assert.match(workflows, /missing_artifact_types/);
});

test("Story 4.4 control-api keeps mutation and read authorization boundaries", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /authorize_critical_action\(&state, &actor, &endpoint, "POST"\)/);
  assert.match(routes, /authorize_report_export_read\(&state, &actor, &endpoint, "GET"\)/);
});

test("Story 4.4 report export endpoints preserve canonical envelope and machine error mapping", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /ReportExportEnvelope/);
  assert.match(routes, /ReportExportMeta/);
  assert.match(routes, /ReportExportEnvelopeError/);
  assert.match(
    routes,
    /ReportingExportReasonCode::InvalidPayload\.code\(\)\s*=>\s*\{?\s*StatusCode::BAD_REQUEST/,
  );
  assert.match(
    routes,
    /ReportingExportReasonCode::Unauthorized\.code\(\)\s*=>\s*StatusCode::FORBIDDEN/,
  );
  assert.match(
    routes,
    /ReportingExportReasonCode::NotFound\.code\(\)\s*=>\s*StatusCode::NOT_FOUND/,
  );
  assert.match(routes, /ReportingExportReasonCode::DependencyUnavailable\.code\(\)/);
  assert.match(routes, /ReportingExportReasonCode::StaleEvidence\.code\(\)/);
  assert.match(routes, /ReportingExportReasonCode::IntegrityMismatch\.code\(\)/);
  assert.match(routes, /ReportingExportReasonCode::PersistenceUnavailable\.code\(\)/);
  assert.match(routes, /"report_export_query_failed"/);
  assert.match(routes, /"report_export_row_decode_failed"/);
  assert.match(routes, /unauthorized_report_export_mutation_attempt_v1/);
  assert.match(routes, /unauthorized_report_export_read_attempt_v1/);
});

test("Story 4.4 export read handlers preserve correlation precedence and fail-closed conflict mapping", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /let effective_correlation_id = query/);
  assert.match(routes, /unwrap_or_else\(\|\| actor\.correlation_id\.clone\(\)\)/);
  assert.match(routes, /correlation_id: effective_correlation_id\.clone\(\)/);
  assert.match(
    routes,
    /report_export_artifact_list_response\([\s\S]*effective_correlation_id,\s*authorization\.timestamp_utc,\s*\)/,
  );
  assert.match(
    routes,
    /report_export_artifact_response\([\s\S]*effective_correlation_id,\s*authorization\.timestamp_utc,\s*\)/,
  );
  assert.match(
    routes,
    /ReportingExportReasonCode::MissingIncidentContext\.code\(\)\s*=>\s*\{?\s*StatusCode::BAD_REQUEST/,
  );
  assert.match(routes, /ReportingExportReasonCode::ArtifactUnavailable\.code\(\)/);
  assert.match(routes, /StatusCode::CONFLICT/);
});
