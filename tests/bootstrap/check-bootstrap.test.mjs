import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import assert from "node:assert/strict";
import { BootstrapError } from "../../tools/bootstrap/check-utils.mjs";
import {
  parseEnvEntries,
  runBootstrapChecks,
  validateEnvironmentTemplate,
} from "../../tools/bootstrap/check-bootstrap.mjs";

const VALID_FIXTURE_ROOT = resolve("tests/fixtures/valid");

test("validateEnvironmentTemplate accepts valid placeholder-only env template", () => {
  const envContent = readFileSync(resolve(VALID_FIXTURE_ROOT, ".env.example"), "utf8");
  const envEntries = parseEnvEntries(envContent);
  assert.doesNotThrow(() => validateEnvironmentTemplate(envEntries));
});

test("validateEnvironmentTemplate rejects missing required placeholders", () => {
  const envEntries = parseEnvEntries("RUNTIME_RUN_AS_NON_ROOT=true\nRUNTIME_UID=10001\n");
  assert.throws(
    () => validateEnvironmentTemplate(envEntries),
    (error) =>
      error instanceof BootstrapError &&
      error.payload.error_code === "BOOTSTRAP_MISSING_PLACEHOLDER",
  );
});

test("validateEnvironmentTemplate rejects plaintext secret-like values", () => {
  const envContent = readFileSync(resolve(VALID_FIXTURE_ROOT, ".env.example"), "utf8");
  const envEntries = parseEnvEntries(envContent);
  envEntries.set("EXECUTION_POLYMARKET_API_KEY", "hardcoded-live-key");
  assert.throws(
    () => validateEnvironmentTemplate(envEntries),
    (error) =>
      error instanceof BootstrapError &&
      error.payload.error_code === "BOOTSTRAP_PLAINTEXT_SECRET",
  );
});

test("runBootstrapChecks produces deterministic entrypoint hash from fixture", () => {
  const summaryA = runBootstrapChecks({ repoRoot: VALID_FIXTURE_ROOT });
  const summaryB = runBootstrapChecks({ repoRoot: VALID_FIXTURE_ROOT });
  assert.equal(summaryA.entrypoint_hash, summaryB.entrypoint_hash);
  assert.equal(summaryA.check_matrix.length, 3);
});
