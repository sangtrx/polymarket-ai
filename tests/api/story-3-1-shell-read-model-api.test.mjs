import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { rmSync, mkdtempSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const PROJECT_ROOT = resolve(".");
const OPERATOR_CONSOLE_FILTER = "operator-console";
const READ_MODEL_SOURCE_PATH = "src/lib/shell/read-models.ts";

let compiledOutputDir;
let resolveShellReadModel;

function parseIsoTimestamp(value) {
  const parsed = Date.parse(value);
  assert.ok(!Number.isNaN(parsed), `expected ISO-8601 timestamp, received: ${value}`);
  return parsed;
}

function loadReadModelApi() {
  if (resolveShellReadModel) {
    return resolveShellReadModel;
  }

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-3-1-read-model-api-"));
  const compileResult = spawnSync(
    "pnpm",
    [
      "--filter",
      OPERATOR_CONSOLE_FILTER,
      "exec",
      "tsc",
      "--pretty",
      "false",
      "--module",
      "commonjs",
      "--target",
      "es2022",
      "--outDir",
      compiledOutputDir,
      READ_MODEL_SOURCE_PATH,
    ],
    { cwd: PROJECT_ROOT, encoding: "utf8" },
  );
  assert.equal(compileResult.status, 0, compileResult.stderr || compileResult.stdout);

  const require = createRequire(import.meta.url);
  const compiledModulePath = resolve(compiledOutputDir, "read-models.js");
  ({ resolveShellReadModel } = require(compiledModulePath));

  assert.equal(
    typeof resolveShellReadModel,
    "function",
    "expected resolveShellReadModel export from compiled module",
  );

  return resolveShellReadModel;
}

test.after(() => {
  if (compiledOutputDir) {
    rmSync(compiledOutputDir, { recursive: true, force: true });
  }
});

test("Story 3.1 API defaults return ready state with deterministic freshness contract", () => {
  const resolveSnapshot = loadReadModelApi();
  const lowerBound = Date.now() - 1_000;

  const snapshot = resolveSnapshot({}, "dashboard.read-model.shell");

  assert.equal(snapshot.dataState, "ready");
  assert.equal(snapshot.p95TargetMs, 2_000);
  assert.equal(snapshot.freshness.source, "dashboard.read-model.shell");
  assert.equal(snapshot.freshness.isStale, false);
  assert.equal(snapshot.evidence, undefined);

  const parsedTimestamp = parseIsoTimestamp(snapshot.freshness.lastUpdatedIso);
  assert.ok(parsedTimestamp >= lowerBound);
  assert.ok(parsedTimestamp <= Date.now() + 1_000);
});

test("Story 3.1 API normalizes invalid shell state and sanitizes invalid source values", () => {
  const resolveSnapshot = loadReadModelApi();

  const snapshot = resolveSnapshot(
    {
      shellState: "unexpected-state",
      source: "invalid/source",
      stale: " yes ",
    },
    "dashboard.read-model.shell",
  );

  assert.equal(snapshot.dataState, "error");
  assert.equal(snapshot.freshness.source, "dashboard.read-model.shell");
  assert.equal(snapshot.freshness.isStale, true);
  assert.equal(snapshot.evidence.errorCode, "SHELL_READ_MODEL_UNAVAILABLE");
  assert.match(snapshot.evidence.message, /read model request failed/i);
  parseIsoTimestamp(snapshot.evidence.timestampIso);
});

test("Story 3.1 API uses first values from query arrays and preserves explicit evidence fields", () => {
  const resolveSnapshot = loadReadModelApi();

  const snapshot = resolveSnapshot(
    {
      shellState: [" unauthorized ", "ready"],
      source: ["  governance.read-model.shell  "],
      at: ["2026-04-06T05:00:00-05:00"],
      errorCode: ["AUTH_401", "IGNORED"],
      message: ["Role lacks governance-read access", "IGNORED"],
    },
    "fallback.source",
  );

  assert.equal(snapshot.dataState, "unauthorized");
  assert.equal(snapshot.freshness.source, "governance.read-model.shell");
  assert.equal(snapshot.freshness.lastUpdatedIso, "2026-04-06T10:00:00.000Z");
  assert.equal(snapshot.freshness.isStale, true);
  assert.equal(snapshot.evidence.errorCode, "AUTH_401");
  assert.equal(snapshot.evidence.message, "Role lacks governance-read access");
  assert.equal(snapshot.evidence.timestampIso, "2026-04-06T10:00:00.000Z");
});

test("Story 3.1 API stale flag parser supports 1/true/yes truthy variants", () => {
  const resolveSnapshot = loadReadModelApi();
  const truthyFlags = ["1", "true", "TRUE", "yes", " Yes "];

  for (const staleValue of truthyFlags) {
    const snapshot = resolveSnapshot(
      { shellState: "ready", stale: staleValue },
      "dashboard.read-model.shell",
    );
    assert.equal(snapshot.dataState, "ready");
    assert.equal(snapshot.freshness.isStale, true);
  }

  const nonTruthySnapshot = resolveSnapshot(
    { shellState: "ready", stale: "0" },
    "dashboard.read-model.shell",
  );
  assert.equal(nonTruthySnapshot.freshness.isStale, false);
});
