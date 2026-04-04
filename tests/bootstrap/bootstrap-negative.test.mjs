import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import test from "node:test";
import assert from "node:assert/strict";
import { FAILURE_FIELDS } from "../../tools/bootstrap/check-utils.mjs";

const FIXTURE_ROOT = resolve("tests/fixtures/valid");
const CHECK_SCRIPT_PATH = resolve("tools/bootstrap/check-bootstrap.mjs");

function withFixtureCopy(mutator) {
  const tempRoot = mkdtempSync(join(tmpdir(), "bootstrap-negative-"));
  cpSync(FIXTURE_ROOT, tempRoot, { recursive: true });
  mutator(tempRoot);
  return tempRoot;
}

function runBootstrapCheck(repoRoot) {
  return spawnSync(process.execPath, [CHECK_SCRIPT_PATH, "--root", repoRoot], {
    encoding: "utf8",
  });
}

function assertMachineReadableFailure(stderrOutput) {
  const payload = JSON.parse(stderrOutput.trim());
  for (const field of FAILURE_FIELDS) {
    assert.ok(Object.hasOwn(payload, field), `missing ${field} in error payload`);
  }
  return payload;
}

test("missing placeholder key fails with machine-readable contract", () => {
  const fixtureRoot = withFixtureCopy((tempRoot) => {
    const envPath = resolve(tempRoot, ".env.example");
    const filtered = readFileSync(envPath, "utf8")
      .split(/\r?\n/)
      .filter((line) => !line.startsWith("CONTROL_API_JWT_ISSUER="))
      .join("\n");
    writeFileSync(envPath, `${filtered}\n`, "utf8");
  });

  try {
    const result = runBootstrapCheck(fixtureRoot);
    assert.notEqual(result.status, 0);
    const failure = assertMachineReadableFailure(result.stderr);
    assert.equal(failure.error_code, "BOOTSTRAP_MISSING_PLACEHOLDER");
    assert.equal(failure.failed_check, "environment_placeholders");
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("plaintext secret-like value fails with machine-readable contract", () => {
  const fixtureRoot = withFixtureCopy((tempRoot) => {
    const envPath = resolve(tempRoot, ".env.example");
    const updated = readFileSync(envPath, "utf8").replace(
      "EXECUTION_POLYMARKET_API_KEY=REPLACE_AT_RUNTIME_SECRET",
      "EXECUTION_POLYMARKET_API_KEY=prod-live-api-key-value",
    );
    writeFileSync(envPath, updated, "utf8");
  });

  try {
    const result = runBootstrapCheck(fixtureRoot);
    assert.notEqual(result.status, 0);
    const failure = assertMachineReadableFailure(result.stderr);
    assert.equal(failure.error_code, "BOOTSTRAP_PLAINTEXT_SECRET");
    assert.equal(failure.failed_check, "environment_placeholders");
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
