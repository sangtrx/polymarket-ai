import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.1 migration scope stays isolated to alpha_hypotheses schema", () => {
  const migration = read("crates/persistence/migrations/20260407160000_alpha_hypotheses.sql");

  assert.match(migration, /CREATE TABLE IF NOT EXISTS alpha_hypotheses/);
  assert.match(migration, /training_window_start_utc < training_window_end_utc/);
  assert.match(migration, /jsonb_typeof\(risk_assumptions_json\) = 'object'/);
  assert.match(migration, /idx_alpha_hypotheses_hypothesis_id_canonical_unique/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS reward_risk_policies/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS market_bucket_profiles/);
});

test("Story 6.1 runbook captures FR6 contract, fail-closed codes, and dataset registry seam assumption", () => {
  const runbook = read("docs/operations/alpha-hypothesis-registry.md");

  assert.match(runbook, /POST \/control\/research\/alpha-hypotheses\/\{hypothesis_id\}/);
  assert.match(runbook, /GET \/control\/research\/alpha-hypotheses\/\{hypothesis_id\}/);
  assert.match(runbook, /`hypothesis_id`/);
  assert.match(runbook, /`feature_set_version`/);
  assert.match(runbook, /`target_regime`/);
  assert.match(runbook, /`expected_edge_source`/);
  assert.match(runbook, /`training_window`/);
  assert.match(runbook, /`risk_assumptions`/);
  assert.match(runbook, /`alpha_hypothesis_dataset_snapshot_unresolved`/);
  assert.match(runbook, /`alpha_hypothesis_dataset_snapshot_unavailable`/);
  assert.match(runbook, /RESEARCH_DATASET_SNAPSHOT_VERSIONS/);
});

test("Story 6.1 cross-runbook links include report-export, reward-risk, and stratification operations", () => {
  const runbook = read("docs/operations/alpha-hypothesis-registry.md");
  const reportExport = read("docs/operations/report-export-workflows.md");
  const rewardRisk = read("docs/operations/reward-risk-policy-operations.md");
  const stratification = read("docs/operations/core-satellite-market-stratification.md");

  assert.match(runbook, /report-export-workflows\.md/);
  assert.match(runbook, /reward-risk-policy-operations\.md/);
  assert.match(runbook, /core-satellite-market-stratification\.md/);
  assert.match(reportExport, /alpha-hypothesis-registry\.md/);
  assert.match(rewardRisk, /alpha-hypothesis-registry\.md/);
  assert.match(stratification, /alpha-hypothesis-registry\.md/);
});

test("Story 6.1 QA command wiring executes Rust and story-scoped API\/E2E checks", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-6-1"/);
  assert.match(packageJson, /cargo test -p domain research::tests::/);
  assert.match(packageJson, /cargo test -p persistence postgres::alpha_hypotheses::tests::/);
  assert.match(packageJson, /cargo test -p research-gateway validation::hypothesis_registry::tests::/);
  assert.match(packageJson, /cargo test -p control-api routes::tests::alpha_hypothesis_/);
  assert.match(packageJson, /tests\/api\/story-6-1\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-6-1\*\.test\.mjs/);
});
