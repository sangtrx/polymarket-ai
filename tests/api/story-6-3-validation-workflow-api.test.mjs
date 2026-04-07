import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.3 exposes authenticated validation-run start/read/list/artifact routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/research\/validation-runs"/);
  assert.match(
    routes,
    /post\(start_validation_run\)\.get\(list_validation_runs\)/,
  );
  assert.match(routes, /\/control\/research\/validation-runs\/\{run_id\}"/);
  assert.match(routes, /get\(read_validation_run\)/);
  assert.match(
    routes,
    /\/control\/research\/validation-runs\/\{run_id\}\/artifacts\/\{stage\}"/,
  );
  assert.match(routes, /get\(read_validation_artifact\)/);
});

test("Story 6.3 payload/query/response contracts follow canonical data/meta/error envelopes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub struct ValidationRunStartPayload \{[\s\S]*pub candidate_id: String,[\s\S]*pub training_entry_observed_metrics: serde_json::Value,[\s\S]*pub stage_inputs: serde_json::Value,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ValidationRunsQuery \{[\s\S]*pub candidate_id: String,[\s\S]*pub limit: Option<i64>,[\s\S]*pub correlation_id: Option<String>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ValidationRunEnvelope<T: Serialize> \{[\s\S]*pub data: Option<T>,[\s\S]*pub meta: ValidationRunMeta,[\s\S]*pub error: Option<ValidationRunEnvelopeError>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub enum ValidationRunData \{[\s\S]*RunDetail[\s\S]*Runs[\s\S]*Artifact[\s\S]*\}/s,
  );
  assert.match(routes, /pub struct ValidationRunEnvelopeError \{/);
});

test("Story 6.3 run-detail payloads expose deterministic artifact and comparison evidence contracts", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /pub enum ValidationRunData \{[\s\S]*RunDetail \{[\s\S]*run: ValidationRunItem,[\s\S]*artifacts: Vec<ValidationArtifactItem>,[\s\S]*comparisons: Vec<ValidationStageComparisonItem>,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ValidationArtifactItem \{[\s\S]*pub stage: String,[\s\S]*pub stage_index: i16,[\s\S]*pub stage_outcome: String,[\s\S]*pub reason_code: String,[\s\S]*pub diagnostics: ValidationDiagnosticsItem,[\s\S]*pub stage_started_at_utc: String,[\s\S]*pub stage_completed_at_utc: String,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ValidationDiagnosticsItem \{[\s\S]*pub out_of_sample_sharpe: f64,[\s\S]*pub max_drawdown: f64,[\s\S]*pub brier_score: Option<f64>,[\s\S]*pub expected_calibration_error: Option<f64>,[\s\S]*pub overfit_indicator: f64,[\s\S]*pub overfit_flag: bool,[\s\S]*\}/s,
  );
  assert.match(
    routes,
    /pub struct ValidationStageComparisonItem \{[\s\S]*pub stage: String,[\s\S]*pub current_run_id: String,[\s\S]*pub previous_run_id: String,[\s\S]*pub reason_code: String,[\s\S]*pub metric_deltas: Vec<ValidationMetricDeltaItem>,[\s\S]*\}/s,
  );
});

test("Story 6.3 status mapping is deterministic and fail-closed", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn validation_run_service_error_status\(code: &str\) -> StatusCode/,
  );
  assert.match(
    routes,
    /ValidationWorkflowReasonCode::InvalidPayload\.code\(\)[\s\S]*StatusCode::BAD_REQUEST/s,
  );
  assert.match(
    routes,
    /ValidationWorkflowReasonCode::UnauthorizedRole\.code\(\)[\s\S]*StatusCode::FORBIDDEN/s,
  );
  assert.match(
    routes,
    /"validation_run_constraint_violation" \| "validation_artifact_constraint_violation"\s*=>\s*\{\s*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /ValidationWorkflowReasonCode::RunNotFound\.code\(\)[\s\S]*ValidationWorkflowReasonCode::ArtifactNotFound\.code\(\)[\s\S]*ValidationWorkflowReasonCode::GateDenied\.code\(\)[\s\S]*ValidationWorkflowReasonCode::StageFailed\.code\(\)[\s\S]*StatusCode::CONFLICT/s,
  );
  assert.match(
    routes,
    /ValidationWorkflowReasonCode::DependencyUnavailable\.code\(\)[\s\S]*ValidationWorkflowReasonCode::StateUnavailable\.code\(\)[\s\S]*ValidationWorkflowReasonCode::PersistenceUnavailable\.code\(\)[\s\S]*StatusCode::SERVICE_UNAVAILABLE/s,
  );
  assert.match(routes, /_ => StatusCode::INTERNAL_SERVER_ERROR/);
});

test("Story 6.3 run/audit handlers preserve actor-action-stage evidence for allow and deny paths", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /fn validation_run_detail_response[\s\S]*"http_method": http_method,[\s\S]*"run_id": run_id,[\s\S]*"candidate_id": candidate_id,[\s\S]*"run_state": run_state,[\s\S]*"artifact_count": artifact_count,[\s\S]*"comparison_count": comparison_count/s,
  );
  assert.match(
    routes,
    /fn validation_artifact_response[\s\S]*"http_method": "GET",[\s\S]*"run_id": run_id,[\s\S]*"stage": stage,[\s\S]*"stage_outcome": stage_outcome/s,
  );
  assert.match(
    routes,
    /fn validation_run_service_error_response[\s\S]*"error_code": error.code,[\s\S]*"failed_stages": error.failed_stages,[\s\S]*outcome: PrivilegedAuditOutcome::AuthorizationDenied/s,
  );
});

test("Story 6.3 control-api state/startup wiring includes validation workflow orchestrator", () => {
  const middleware = read("services/control-api/src/middleware/mod.rs");
  const main = read("services/control-api/src/main.rs");

  assert.match(
    middleware,
    /pub research_validation_workflow_orchestrator: Arc<dyn ValidationWorkflowRunOrchestrator>/,
  );
  assert.match(
    middleware,
    /with_research_validation_workflow_orchestrator\(/,
  );
  assert.match(main, /ValidationWorkflowRunService::postgres\(/);
  assert.match(main, /with_research_validation_workflow_orchestrator\(/);
});

test("Story 6.3 research-gateway exports validation workflow orchestration seams", () => {
  const validationMod = read("services/research-gateway/src/validation/mod.rs");
  const workflowRuns = read(
    "services/research-gateway/src/validation/workflow_runs.rs",
  );

  assert.match(validationMod, /pub mod workflow_runs;/);
  assert.match(
    workflowRuns,
    /pub trait ValidationWorkflowRunOrchestrator: Send \+ Sync/,
  );
  assert.match(workflowRuns, /fn start_validation_run\(/);
  assert.match(workflowRuns, /fn read_validation_run\(/);
  assert.match(workflowRuns, /fn list_validation_runs\(/);
  assert.match(workflowRuns, /fn read_validation_artifact\(/);
  assert.match(workflowRuns, /evaluate_training_entry_gates\(/);
});

test("Story 6.3 unauthorized validation-run denials emit machine-readable security signals", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /unauthorized_validation_run_read_attempt_v1/);
  assert.match(routes, /unauthorized_validation_run_mutation_attempt_v1/);
});
