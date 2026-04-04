import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import test from "node:test";
import assert from "node:assert/strict";

const FIXTURE_ROOT = resolve("tests/fixtures/valid");
const CHECK_SCRIPT_PATH = resolve("tools/bootstrap/check-bootstrap.mjs");
const SECURITY_SCAN_SCRIPT_PATH = resolve("tools/bootstrap/security-scan.mjs");

function withFixtureCopy(mutator) {
  const tempRoot = mkdtempSync(join(tmpdir(), "bootstrap-e2e-"));
  cpSync(FIXTURE_ROOT, tempRoot, { recursive: true });
  writeFileSync(resolve(tempRoot, "pnpm-lock.yaml"), "lockfileVersion: '9.0'\n", "utf8");
  mutator(tempRoot);
  return tempRoot;
}

function runNodeScript(scriptPath, args) {
  return spawnSync(process.execPath, [scriptPath, ...args], {
    encoding: "utf8",
  });
}

function parseSummary(output) {
  const lines = output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  return JSON.parse(lines.at(-1));
}

test("bootstrap baseline e2e writes traceable evidence with matching hashes", () => {
  const fixtureRoot = withFixtureCopy(() => {});
  const bootstrapEvidencePath = "bootstrap-evidence/bootstrap-check.json";
  const securityEvidencePath = "bootstrap-evidence/security-summary.json";

  try {
    const bootstrapResult = runNodeScript(CHECK_SCRIPT_PATH, [
      "--root",
      fixtureRoot,
      "--evidence-file",
      bootstrapEvidencePath,
    ]);
    assert.equal(bootstrapResult.status, 0, bootstrapResult.stderr);
    const bootstrapSummary = parseSummary(bootstrapResult.stdout);

    const securityResult = runNodeScript(SECURITY_SCAN_SCRIPT_PATH, [
      "--root",
      fixtureRoot,
      "--evidence-file",
      securityEvidencePath,
    ]);
    assert.equal(securityResult.status, 0, securityResult.stderr);
    const securitySummary = parseSummary(securityResult.stdout);

    assert.equal(
      securitySummary.baseline_entrypoint_hash,
      bootstrapSummary.entrypoint_hash,
    );
    assert.deepEqual(
      securitySummary.check_matrix.map((item) => item.check),
      ["dependency_lockfiles", "secret_material_scan"],
    );
    assert.ok(securitySummary.check_matrix.every((item) => item.status === "pass"));

    const bootstrapEvidence = JSON.parse(
      readFileSync(resolve(fixtureRoot, bootstrapEvidencePath), "utf8"),
    );
    const securityEvidence = JSON.parse(
      readFileSync(resolve(fixtureRoot, securityEvidencePath), "utf8"),
    );

    assert.equal(bootstrapEvidence.entrypoint_hash, bootstrapSummary.entrypoint_hash);
    assert.equal(
      securityEvidence.baseline_entrypoint_hash,
      bootstrapSummary.entrypoint_hash,
    );
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("bootstrap baseline e2e surfaces machine-readable blocker on secret leakage", () => {
  const fixtureRoot = withFixtureCopy((tempRoot) => {
    const injectedPath = resolve(tempRoot, "apps/operator-console/leaky-key.txt");
    const privateKeyLiteral =
      "-----BEGIN " +
      "PRIVATE KEY-----\n" +
      "leaky-material\n" +
      "-----END PRIVATE KEY-----\n";
    writeFileSync(
      injectedPath,
      privateKeyLiteral,
      "utf8",
    );
  });

  try {
    const result = runNodeScript(SECURITY_SCAN_SCRIPT_PATH, ["--root", fixtureRoot]);
    assert.notEqual(result.status, 0);
    const failure = JSON.parse(result.stderr.trim());
    assert.equal(failure.error_code, "SECURITY_PRIVATE_KEY_LITERAL");
    assert.equal(failure.failed_check, "secret-material-scan");
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
