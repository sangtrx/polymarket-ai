import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { rmSync, mkdtempSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const PROJECT_ROOT = resolve(".");
const OPERATOR_CONSOLE_FILTER = "operator-console";
const CONTROL_ACTION_SOURCE_PATH = "src/lib/risk/control-actions.ts";
const RISK_POSTURE_SOURCE_PATH = "src/lib/risk/posture.ts";

let compiledOutputDir;
let controlActionsModule;
let postureModule;

function parseIsoTimestamp(value) {
  const parsed = Date.parse(value);
  assert.ok(!Number.isNaN(parsed), `expected ISO-8601 timestamp, received: ${value}`);
  return parsed;
}

function findFileRecursively(rootDir, fileName) {
  const queue = [rootDir];
  while (queue.length > 0) {
    const nextDir = queue.shift();
    const entries = readdirSync(nextDir, { withFileTypes: true });
    for (const entry of entries) {
      const entryPath = join(nextDir, entry.name);
      if (entry.isDirectory()) {
        queue.push(entryPath);
      } else if (entry.isFile() && entry.name === fileName) {
        return entryPath;
      }
    }
  }
  return undefined;
}

function loadCompiledModules() {
  if (controlActionsModule && postureModule) {
    return { controlActionsModule, postureModule };
  }

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-3-2-safety-control-api-"));
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
      CONTROL_ACTION_SOURCE_PATH,
      RISK_POSTURE_SOURCE_PATH,
    ],
    { cwd: PROJECT_ROOT, encoding: "utf8" },
  );
  assert.equal(compileResult.status, 0, compileResult.stderr || compileResult.stdout);

  const controlActionsPath = findFileRecursively(compiledOutputDir, "control-actions.js");
  const posturePath = findFileRecursively(compiledOutputDir, "posture.js");
  assert.ok(controlActionsPath, "expected compiled control-actions.js output");
  assert.ok(posturePath, "expected compiled posture.js output");

  const require = createRequire(import.meta.url);
  controlActionsModule = require(controlActionsPath);
  postureModule = require(posturePath);

  return { controlActionsModule, postureModule };
}

test.after(() => {
  if (compiledOutputDir) {
    rmSync(compiledOutputDir, { recursive: true, force: true });
  }
});

test("Story 3.2 API maps accepted emergency control response with canonical evidence", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeEmergencyControlAction } = controlActionsModule;

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 202,
      json: async () => ({
        status: "accepted",
        action_id: "action::pause-001",
        action: "pause",
        source: "manual",
        trigger_source: "operator_command",
        resulting_mode: "paused",
        reason_code: "emergency_control_pause_activated",
        correlation_id: "corr-001",
        timestamp_utc: "2026-04-06T06:11:35.000Z",
        audit_reference: "ticket-123",
      }),
    };
  };

  const result = await invokeEmergencyControlAction({
    baseUrl: "http://127.0.0.1:8080",
    action: "pause",
    auditReference: "ticket-123",
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(requests[0].url, "http://127.0.0.1:8080/control/emergency/pause");
  assert.equal(requests[0].init.method, "POST");
  assert.match(requests[0].init.body, /"audit_reference":"ticket-123"/);
  assert.equal(result.actionId, "action::pause-001");
  assert.equal(result.resultingMode, "paused");
  assert.equal(result.reasonCode, "emergency_control_pause_activated");
  assert.equal(result.correlationId, "corr-001");
  assert.equal(result.auditReference, "ticket-123");
});

test("Story 3.2 API surfaces machine-readable errors with no success-shaped fallback", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeEmergencyControlAction } = controlActionsModule;

  const fetchImpl = async () => ({
    ok: false,
    status: 403,
    json: async () => ({
      error_code: "authorization_denied",
      message: "role not permitted",
      action: "emergency_control_pause",
      actor_id: "reader-1",
      role: "read_only_analytics",
      correlation_id: "corr-denied",
      timestamp_utc: "2026-04-06T06:11:36.000Z",
      endpoint: "/control/emergency/pause",
    }),
  });

  await assert.rejects(
    invokeEmergencyControlAction({
      baseUrl: "http://127.0.0.1:8080",
      action: "pause",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 403);
      assert.equal(error.errorCode, "authorization_denied");
      assert.equal(error.action, "emergency_control_pause");
      assert.equal(error.correlationId, "corr-denied");
      return true;
    },
  );
});

test("Story 3.2 API preserves dependency-unavailable errors for explicit operator remediation", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeEmergencyControlAction } = controlActionsModule;

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: false,
      status: 503,
      json: async () => ({
        error_code: "emergency_control_dependency_unavailable",
        message: "control-api dependency unavailable",
        action: "emergency_control_reduce_only",
        correlation_id: "corr-503",
        endpoint: "/control/emergency/reduce-only",
        timestamp_utc: "2026-04-06T06:11:36.900Z",
      }),
    };
  };

  await assert.rejects(
    invokeEmergencyControlAction({
      baseUrl: "http://127.0.0.1:8080",
      action: "reduce-only",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 503);
      assert.equal(error.errorCode, "emergency_control_dependency_unavailable");
      assert.equal(error.action, "emergency_control_reduce_only");
      assert.equal(error.correlationId, "corr-503");
      assert.equal(error.endpoint, "/control/emergency/reduce-only");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/emergency/reduce-only",
  );
  assert.equal(requests[0].init.method, "POST");
});

test("Story 3.2 API action-result lookup uses canonical endpoint and response mapping", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { getEmergencyControlActionResult } = controlActionsModule;

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 200,
      json: async () => ({
        status: "accepted",
        action_id: "action::reduce-only-001",
        action: "reduce_only",
        source: "manual",
        trigger_source: "operator_command",
        resulting_mode: "reduce_only",
        reason_code: "emergency_control_reduce_only_activated",
        correlation_id: "corr-002",
        timestamp_utc: "2026-04-06T06:11:37.000Z",
        audit_reference: "ticket-456",
      }),
    };
  };

  const result = await getEmergencyControlActionResult({
    baseUrl: "http://127.0.0.1:8080",
    actionId: "action::reduce-only-001",
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/emergency/actions/action%3A%3Areduce-only-001",
  );
  assert.equal(requests[0].init.method, "GET");
  assert.equal(result.action, "reduce_only");
  assert.equal(result.resultingMode, "reduce_only");
});

test("Story 3.2 API action-result lookup surfaces canonical not-found errors", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { getEmergencyControlActionResult } = controlActionsModule;

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: false,
      status: 404,
      json: async () => ({
        error_code: "emergency_control_action_not_found",
        message: "action id was not found",
        correlation_id: "corr-404",
        endpoint: "/control/emergency/actions/action%3A%3Amissing-001",
        timestamp_utc: "2026-04-06T06:11:37.400Z",
      }),
    };
  };

  await assert.rejects(
    getEmergencyControlActionResult({
      baseUrl: "http://127.0.0.1:8080",
      actionId: "action::missing-001",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 404);
      assert.equal(error.errorCode, "emergency_control_action_not_found");
      assert.equal(error.action, "emergency_control_action_query");
      assert.equal(
        error.endpoint,
        "/control/emergency/actions/action%3A%3Amissing-001",
      );
      assert.equal(error.correlationId, "corr-404");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/emergency/actions/action%3A%3Amissing-001",
  );
  assert.equal(requests[0].init.method, "GET");
});

test("Story 3.2 API rejects malformed action IDs before making result lookup requests", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { getEmergencyControlActionResult } = controlActionsModule;

  await assert.rejects(
    getEmergencyControlActionResult({
      baseUrl: "http://127.0.0.1:8080",
      actionId: "../unsafe-path",
      fetchImpl: async () => {
        throw new Error("fetch should not be called for invalid action IDs");
      },
    }),
    (error) => {
      assert.equal(error.status, 400);
      assert.equal(error.errorCode, "emergency_control_invalid_action_id");
      return true;
    },
  );
});

test("Story 3.2 posture resolver enforces deterministic defaults and readiness-gated resume", () => {
  const { postureModule } = loadCompiledModules();
  const { resolveRiskPostureViewModel } = postureModule;

  const critical = resolveRiskPostureViewModel(
    {
      riskState: "critical",
      reasonCode: "emergency_control_pause_activated",
      mode: "paused",
      actionId: "action::critical-001",
      correlationId: "corr-critical-001",
      auditReference: "ticket-critical",
    },
    {
      dataState: "ready",
      freshness: {
        lastUpdatedIso: "2026-04-06T06:11:38.000Z",
        source: "dashboard.read-model.shell",
        isStale: false,
      },
      p95TargetMs: 2000,
    },
  );

  assert.equal(critical.posture, "critical");
  assert.equal(critical.resumeState, "gated");
  assert.equal(critical.interactionBudget, 2);
  assert.equal(critical.acknowledgementTargetMs, 1_000);
  assert.equal(critical.reflectionTargetMs, 5_000);
  assert.equal(critical.confirmationTargetMs, 10_000);
  assert.equal(critical.evidence.actionId, "action::critical-001");
  assert.equal(critical.evidence.correlationId, "corr-critical-001");
  assert.equal(critical.evidence.auditReference, "ticket-critical");

  const fallback = resolveRiskPostureViewModel(
    {},
    {
      dataState: "unauthorized",
      freshness: {
        lastUpdatedIso: "2026-04-06T06:11:39.000Z",
        source: "governance.read-model.shell",
        isStale: true,
      },
      p95TargetMs: 2000,
    },
  );

  assert.equal(fallback.posture, "locked-safe");
  assert.equal(fallback.ariaLive, "assertive");
});

test("Story 3.2 posture resolver maps stale read-model inputs to warning posture deterministically", () => {
  const { postureModule } = loadCompiledModules();
  const { resolveRiskPostureViewModel } = postureModule;

  const warning = resolveRiskPostureViewModel(
    {
      mode: "reduce-only",
      riskSource: "invalid/source",
    },
    {
      dataState: "ready",
      freshness: {
        lastUpdatedIso: "2026-04-06T06:11:40.000Z",
        source: "dashboard.read-model.shell",
        isStale: true,
      },
      p95TargetMs: 2000,
    },
  );

  assert.equal(warning.posture, "warning");
  assert.equal(warning.ariaLive, "polite");
  assert.equal(warning.evidence.source, "dashboard.read-model.shell");
  assert.equal(warning.evidence.resultingMode, "reduce_only");
  assert.equal(warning.evidence.reasonCode, "risk_posture_warning");
  assert.equal(warning.resumeState, "gated");
});
