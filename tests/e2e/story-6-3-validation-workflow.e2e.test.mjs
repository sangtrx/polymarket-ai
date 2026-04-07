import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.3 migration scope stays isolated to validation_runs and validation_artifacts", () => {
  const migration = read(
    "crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql",
  );

  assert.match(migration, /CREATE TABLE IF NOT EXISTS validation_runs/);
  assert.match(migration, /CREATE TABLE IF NOT EXISTS validation_artifacts/);
  assert.match(
    migration,
    /run_state IN \('running', 'completed', 'blocked', 'failed'\)/,
  );
  assert.match(
    migration,
    /stage IN \('quality', 'labeling', 'purged_cv', 'cpcv', 'overfit_diagnostics'\)/,
  );
  assert.match(migration, /stage_index BETWEEN 1 AND 5/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS alpha_hypotheses/);
  assert.doesNotMatch(
    migration,
    /CREATE TABLE IF NOT EXISTS validation_gate_policies/,
  );
});

test("Story 6.3 runbook captures FR7 workflow, FR44 diagnostics, and cross-runbook links", () => {
  const runbook = read(
    "docs/operations/alpha-validation-workflow-and-diagnostics.md",
  );

  assert.match(runbook, /POST \/control\/research\/validation-runs/);
  assert.match(runbook, /GET \/control\/research\/validation-runs\/\{run_id\}/);
  assert.match(
    runbook,
    /GET \/control\/research\/validation-runs\/\{run_id\}\/artifacts\/\{stage\}/,
  );
  assert.match(
    runbook,
    /quality[\s\S]*labeling[\s\S]*purged_cv[\s\S]*cpcv[\s\S]*overfit_diagnostics/s,
  );
  assert.match(runbook, /out_of_sample_sharpe/);
  assert.match(runbook, /max_drawdown/);
  assert.match(runbook, /brier_score/);
  assert.match(runbook, /expected_calibration_error/);
  assert.match(runbook, /validation_run_dependency_unavailable/);
  assert.match(runbook, /validation_run_state_unavailable/);
  assert.match(runbook, /validation_run_persistence_unavailable/);
  assert.match(runbook, /alpha-hypothesis-registry\.md/);
  assert.match(runbook, /alpha-validation-gate-policies\.md/);
  assert.match(runbook, /reward-risk-policy-operations\.md/);
  assert.match(runbook, /risk-limit-policy-operations\.md/);
  assert.match(runbook, /report-export-workflows\.md/);
});

test("Story 6.3 orchestration enforces FR43 precheck and fail-closed stage progression semantics", () => {
  const workflowRuns = read(
    "services/research-gateway/src/validation/workflow_runs.rs",
  );
  const runbook = read(
    "docs/operations/alpha-validation-workflow-and-diagnostics.md",
  );

  assert.match(workflowRuns, /let gate_evaluation = evaluate_training_entry_gates\(/);
  assert.match(workflowRuns, /for stage in ValidationWorkflowStage::ordered\(\)/);
  assert.match(
    workflowRuns,
    /let Some\(stage_input\) = stage_inputs\.get\(stage\.as_str\(\)\) else \{[\s\S]*terminal_state = ValidationWorkflowRunState::Blocked[\s\S]*ValidationWorkflowReasonCode::DependencyUnavailable/s,
  );
  assert.match(
    workflowRuns,
    /ValidationWorkflowStageOutcome::Failed => \{[\s\S]*terminal_state = ValidationWorkflowRunState::Failed[\s\S]*break;/s,
  );
  assert.match(
    workflowRuns,
    /ValidationWorkflowStageOutcome::Blocked => \{[\s\S]*terminal_state = ValidationWorkflowRunState::Blocked[\s\S]*break;/s,
  );
  assert.match(runbook, /No success-shaped fallback is emitted/);
});

test("Story 6.3 deterministic comparison baseline and stage timing contracts stay explicit", () => {
  const workflowRuns = read(
    "services/research-gateway/src/validation/workflow_runs.rs",
  );
  const runbook = read(
    "docs/operations/alpha-validation-workflow-and-diagnostics.md",
  );

  assert.match(
    workflowRuns,
    /if candidate_started_at < current_started_at \{[\s\S]*previous_completed_run = Some\(candidate_run\);[\s\S]*break;/s,
  );
  assert.match(workflowRuns, /fn validation_run_read_does_not_compare_against_newer_runs\(\)/);
  assert.match(
    workflowRuns,
    /comparisons\.sort_by\(\|left, right\| left\.stage\.stage_index\(\)\.cmp\(&right\.stage\.stage_index\(\)\)\);/,
  );
  assert.match(
    workflowRuns,
    /fn stage_timestamp\(base: OffsetDateTime, stage_index: i16, additional_seconds: i64\) -> String \{[\s\S]*Duration::seconds\(i64::from\(stage_index - 1\) \* 2 \+ additional_seconds\)/s,
  );
  assert.match(runbook, /latest completed run before current `started_at_utc`/);
  assert.match(runbook, /deterministic by stage index \(`quality -> \.\.\. -> overfit_diagnostics`\)/);
});

test("Story 6.3 QA command wiring executes Rust plus API/E2E validation-run checks", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-6-3"/);
  assert.match(
    packageJson,
    /cargo test -p domain research::tests::validation_run_/,
  );
  assert.match(
    packageJson,
    /cargo test -p persistence postgres::validation_runs::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p persistence postgres::validation_artifacts::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p research-gateway validation::workflow_runs::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p control-api routes::tests::validation_run_/,
  );
  assert.match(packageJson, /tests\/api\/story-6-3\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-6-3\*\.test\.mjs/);
});
