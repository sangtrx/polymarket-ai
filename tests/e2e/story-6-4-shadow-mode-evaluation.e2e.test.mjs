import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.4 migration scope stays isolated to shadow_evaluations", () => {
  const migration = read(
    "crates/persistence/migrations/20260407210000_shadow_evaluations.sql",
  );

  assert.match(migration, /CREATE TABLE IF NOT EXISTS shadow_evaluations/);
  assert.match(
    migration,
    /evaluation_state IN \('running', 'completed', 'denied', 'failed'\)/,
  );
  assert.match(
    migration,
    /idx_shadow_evaluations_candidate_lookup/,
  );
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS validation_runs/);
  assert.doesNotMatch(
    migration,
    /CREATE TABLE IF NOT EXISTS validation_artifacts/,
  );
});

test("Story 6.4 runbook captures FR9 flows, read-only guarantees, and cross-links", () => {
  const runbook = read("docs/operations/alpha-shadow-mode-evaluation.md");

  assert.match(runbook, /POST \/control\/research\/shadow-evaluations/);
  assert.match(
    runbook,
    /GET \/control\/research\/shadow-evaluations\/\{evaluation_id\}/,
  );
  assert.match(
    runbook,
    /GET \/control\/research\/shadow-evaluations\?candidate_id=\{candidate_id\}/,
  );
  assert.match(runbook, /no live order placement\/cancel pathways/);
  assert.match(runbook, /shadow_simulation_read_only_enforced/);
  assert.match(runbook, /shadow_evaluation_dependency_unavailable/);
  assert.match(runbook, /shadow_evaluation_state_unavailable/);
  assert.match(runbook, /shadow_evaluation_persistence_unavailable/);
  assert.match(runbook, /alpha-validation-workflow-and-diagnostics\.md/);
  assert.match(runbook, /alpha-validation-gate-policies\.md/);
});

test("Story 6.4 orchestration enforces validation evidence precheck and read-only simulation behavior", () => {
  const shadowMode = read("services/research-gateway/src/validation/shadow_mode.rs");

  assert.match(shadowMode, /load_completed_validation_run\(/);
  assert.match(shadowMode, /list_validation_artifacts_by_run\(/);
  assert.match(shadowMode, /trait MarketContextPort/);
  assert.match(shadowMode, /trait ShadowSimulationPort/);
  assert.match(shadowMode, /shadow_simulation_read_only_enforced/);
  assert.doesNotMatch(shadowMode, /submit_order|cancel_order|cancel_all/);
});

test("Story 6.4 persistence adapter enforces deterministic list ordering and boundary-safe filters", () => {
  const persistence = read("crates/persistence/src/postgres/shadow_evaluations.rs");

  assert.match(
    persistence,
    /ORDER BY started_at_utc DESC, evaluation_id ASC/,
  );
  assert.match(
    persistence,
    /if limit <= 0 \{[\s\S]*"limit must be greater than 0"/s,
  );
  assert.match(
    persistence,
    /parse_shadow_utc_timestamp\(/,
  );
});

test("Story 6.4 list orchestration fails closed on malformed time-window boundaries with deny telemetry", () => {
  const shadowMode = read("services/research-gateway/src/validation/shadow_mode.rs");

  assert.match(
    shadowMode,
    /normalize_optional_timestamp\("started_after_utc", input\.started_after_utc\.as_deref\(\)\)/,
  );
  assert.match(
    shadowMode,
    /normalize_optional_timestamp\("started_before_utc", input\.started_before_utc\.as_deref\(\)\)/,
  );
  assert.match(
    shadowMode,
    /if let \(Some\(started_after\), Some\(started_before\)\)/,
  );
  assert.match(
    shadowMode,
    /started_before_utc must be greater than started_after_utc/,
  );
  assert.match(
    shadowMode,
    /emit_shadow_evaluation_telemetry\([\s\S]*"shadow_evaluation_list_v1"[\s\S]*"shadow_evaluation_list"[\s\S]*"deny"[\s\S]*error\.code/s,
  );
});

test("Story 6.4 list orchestration preserves deterministic limit/repository contracts and allow telemetry", () => {
  const shadowMode = read("services/research-gateway/src/validation/shadow_mode.rs");

  assert.match(shadowMode, /let limit = input\.limit\.unwrap_or\(25\)\.clamp\(1, 200\);/);
  assert.match(
    shadowMode,
    /list_by_candidate\([\s\S]*&normalized_candidate_id,[\s\S]*normalized_started_after\.as_deref\(\),[\s\S]*normalized_started_before\.as_deref\(\),[\s\S]*limit,[\s\S]*\)/s,
  );
  assert.match(
    shadowMode,
    /emit_shadow_evaluation_telemetry\([\s\S]*"shadow_evaluation_list_v1"[\s\S]*"shadow_evaluation_list"[\s\S]*"allow"[\s\S]*ShadowEvaluationReasonCode::EvaluationListed\.code\(\)/s,
  );
});

test("Story 6.4 QA command wiring executes Rust and story-scoped API/E2E checks", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-6-4"/);
  assert.match(
    packageJson,
    /cargo test -p domain research::tests::shadow_evaluation_/,
  );
  assert.match(
    packageJson,
    /cargo test -p persistence postgres::shadow_evaluations::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p research-gateway validation::shadow_mode::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p control-api routes::tests::shadow_evaluation_/,
  );
  assert.match(packageJson, /tests\/api\/story-6-4\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-6-4\*\.test\.mjs/);
});
