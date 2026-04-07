import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.2 migration scope stays isolated to validation_gate_policies schema", () => {
  const migration = read(
    "crates/persistence/migrations/20260407173000_validation_gate_policies.sql",
  );

  assert.match(migration, /CREATE TABLE IF NOT EXISTS validation_gate_policies/);
  assert.match(
    migration,
    /gate_type IN \(\s*'forward_bias',\s*'data_leakage',\s*'regime_survivability',\s*'data_quality'/s,
  );
  assert.match(
    migration,
    /stage_scope IN \('training', 'promotion', 'training_and_promotion'\)/,
  );
  assert.match(
    migration,
    /idx_validation_gate_policies_policy_key_canonical_unique/,
  );
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS alpha_hypotheses/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS reward_risk_policies/);
});

test("Story 6.2 runbook captures FR43 gate catalog, fail-closed evaluation, and cross-links", () => {
  const runbook = read("docs/operations/alpha-validation-gate-policies.md");

  assert.match(
    runbook,
    /POST \/control\/research\/validation-gate-policies\/\{policy_key\}/,
  );
  assert.match(
    runbook,
    /GET \/control\/research\/validation-gate-policies\/\{policy_key\}/,
  );
  assert.match(
    runbook,
    /POST \/control\/research\/validation-gate-policies\/evaluate\/\{stage\}/,
  );
  assert.match(runbook, /`forward_bias`/);
  assert.match(runbook, /`data_leakage`/);
  assert.match(runbook, /`regime_survivability`/);
  assert.match(runbook, /`data_quality`/);
  assert.match(runbook, /`validation_gate_dependency_unavailable`/);
  assert.match(runbook, /`validation_gate_state_unavailable`/);
  assert.match(runbook, /alpha-hypothesis-registry\.md/);
  assert.match(runbook, /reward-risk-policy-operations\.md/);
  assert.match(runbook, /risk-limit-policy-operations\.md/);
  assert.match(runbook, /report-export-workflows\.md/);
});

test("Story 6.2 runbook documents deterministic comparator boundaries and fail-closed fallback posture", () => {
  const runbook = read("docs/operations/alpha-validation-gate-policies.md");

  assert.match(runbook, /`lt`: `observed < threshold`/);
  assert.match(runbook, /`lte`: `observed <= threshold`/);
  assert.match(runbook, /`gt`: `observed > threshold`/);
  assert.match(runbook, /`gte`: `observed >= threshold`/);
  assert.match(runbook, /`lt`\/`gt` reject equality\./);
  assert.match(runbook, /`lte`\/`gte` accept equality\./);
  assert.match(runbook, /`validation_gate_missing_mandatory_policy`/);
  assert.match(runbook, /`validation_gate_failed`/);
  assert.match(runbook, /No success-shaped fallback is emitted/);
});

test("Story 6.2 QA command wiring executes Rust and story-scoped API/E2E checks", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-6-2"/);
  assert.match(packageJson, /cargo test -p domain research::tests::validation_gate_/);
  assert.match(
    packageJson,
    /cargo test -p persistence postgres::validation_gate_policies::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p research-gateway validation::gate_policies::tests::/,
  );
  assert.match(packageJson, /cargo test -p control-api routes::tests::validation_gate_/);
  assert.match(packageJson, /tests\/api\/story-6-2\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-6-2\*\.test\.mjs/);
});
