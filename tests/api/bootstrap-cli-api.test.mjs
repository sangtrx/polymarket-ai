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
  const tempRoot = mkdtempSync(join(tmpdir(), "bootstrap-cli-api-"));
  cpSync(FIXTURE_ROOT, tempRoot, { recursive: true });
  mutator(tempRoot);
  return tempRoot;
}

function runBootstrapCheck(repoRoot) {
  return spawnSync(process.execPath, [CHECK_SCRIPT_PATH, "--root", repoRoot], {
    encoding: "utf8",
  });
}

function parseJsonLines(output) {
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line) => JSON.parse(line));
}

test("bootstrap check CLI success emits machine-readable summary payload", () => {
  const result = runBootstrapCheck(FIXTURE_ROOT);
  assert.equal(result.status, 0, result.stderr);

  const records = parseJsonLines(result.stdout);
  assert.ok(records.length >= 4, "expected check-line telemetry plus summary payload");

  const summary = records.at(-1);
  assert.deepEqual(summary.artifact_flow, ["ci:rust", "ci:web", "ci:security"]);
  assert.equal(summary.check_matrix.length, 3);
  assert.deepEqual(
    summary.check_matrix.map((item) => item.check),
    ["required_paths", "environment_template", "reproducible_entrypoints"],
  );
  assert.ok(summary.check_matrix.every((item) => item.status === "pass"));
  assert.match(summary.entrypoint_hash, /^[a-f0-9]{64}$/);
  assert.match(summary.timestamp_utc, /^\d{4}-\d{2}-\d{2}T/);
});

test("bootstrap check CLI failure emits machine-readable error payload", () => {
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

    const payload = JSON.parse(result.stderr.trim());
    for (const field of FAILURE_FIELDS) {
      assert.ok(Object.hasOwn(payload, field), `missing ${field} in error payload`);
    }
    assert.equal(payload.error_code, "BOOTSTRAP_MISSING_PLACEHOLDER");
    assert.equal(payload.failed_check, "environment_placeholders");
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
