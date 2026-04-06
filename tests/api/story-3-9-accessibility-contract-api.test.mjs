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

let compiledOutputDir;
let controlActionsModule;

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

function loadCompiledModule() {
  if (controlActionsModule) {
    return controlActionsModule;
  }

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-3-9-accessibility-api-"));
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
    ],
    { cwd: PROJECT_ROOT, encoding: "utf8" },
  );
  assert.equal(compileResult.status, 0, compileResult.stderr || compileResult.stdout);

  const compiledPath = findFileRecursively(compiledOutputDir, "control-actions.js");
  assert.ok(compiledPath, "expected compiled control-actions.js output");

  const require = createRequire(import.meta.url);
  controlActionsModule = require(compiledPath);
  return controlActionsModule;
}

function acceptedReadinessPayload(overrides = {}) {
  return {
    status: "accepted",
    action: "recovery_readiness_evaluate",
    run_id: "recovery::gate::default::corr-1::20260406120000",
    readiness_status: "approved",
    reason_code: "recovery_resume_approved",
    profile_key: "default",
    actor_id: "ops-1",
    actor_role: "operational_control",
    correlation_id: "corr-1",
    requested_at_utc: "2026-04-06T12:00:00.000Z",
    evaluated_at_utc: "2026-04-06T12:00:01.000Z",
    reconciliation_run_id: "recon-1",
    approved_checksum:
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    computed_checksum:
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    failing_gate_codes: [],
    gate_outcomes: [
      {
        gate: "freshness",
        passed: true,
        reason_code: "recovery_freshness_pass",
        trigger: "freshness <= 30s",
        context: "freshness_age_seconds=10",
        action: "allow freshness gate",
        verification: "freshness evidence verified",
      },
    ],
    recommended_next_action: "Execute controlled recovery resume.",
    timestamp_utc: "2026-04-06T12:00:01.000Z",
    ...overrides,
  };
}

test.after(() => {
  if (compiledOutputDir) {
    rmSync(compiledOutputDir, { recursive: true, force: true });
  }
});

test("Story 3.9 control-actions rejects accepted emergency payload missing timestamp_utc", async () => {
  const { invokeEmergencyControlAction } = loadCompiledModule();

  await assert.rejects(
    invokeEmergencyControlAction({
      baseUrl: "http://127.0.0.1:8080",
      action: "pause",
      fetchImpl: async () => ({
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
        }),
      }),
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "emergency_control_contract_mismatch");
      assert.match(error.message, /timestamp_utc/i);
      return true;
    },
  );
});

test("Story 3.9 control-actions rejects accepted emergency payload with malformed timestamp_utc", async () => {
  const { invokeEmergencyControlAction } = loadCompiledModule();

  await assert.rejects(
    invokeEmergencyControlAction({
      baseUrl: "http://127.0.0.1:8080",
      action: "pause",
      fetchImpl: async () => ({
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
          timestamp_utc: "not-an-iso-timestamp",
        }),
      }),
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "emergency_control_contract_mismatch");
      assert.match(error.message, /timestamp_utc/i);
      return true;
    },
  );
});

test("Story 3.9 control-actions rejects accepted emergency payload with non-ISO timestamp_utc format", async () => {
  const { invokeEmergencyControlAction } = loadCompiledModule();

  await assert.rejects(
    invokeEmergencyControlAction({
      baseUrl: "http://127.0.0.1:8080",
      action: "pause",
      fetchImpl: async () => ({
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
          timestamp_utc: "Mon, 06 Apr 2026 12:00:05 GMT",
        }),
      }),
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "emergency_control_contract_mismatch");
      assert.match(error.message, /timestamp_utc/i);
      return true;
    },
  );
});

test("Story 3.9 control-actions rejects accepted emergency payload missing correlation_id evidence", async () => {
  const { invokeEmergencyControlAction } = loadCompiledModule();

  await assert.rejects(
    invokeEmergencyControlAction({
      baseUrl: "http://127.0.0.1:8080",
      action: "pause",
      fetchImpl: async () => ({
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
          timestamp_utc: "2026-04-06T12:00:05.000Z",
        }),
      }),
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "emergency_control_contract_mismatch");
      assert.match(error.message, /correlation_id/i);
      return true;
    },
  );
});

test("Story 3.9 control-actions preserves backend error details when error timestamp evidence is malformed", async () => {
  const { invokeEmergencyControlAction } = loadCompiledModule();

  await assert.rejects(
    invokeEmergencyControlAction({
      baseUrl: "http://127.0.0.1:8080",
      action: "pause",
      fetchImpl: async () => ({
        ok: false,
        status: 503,
        json: async () => ({
          error_code: "dependency_unavailable",
          message: "dependency unavailable",
          action: "pause",
          correlation_id: "corr-001",
          timestamp_utc: "malformed-timestamp",
        }),
      }),
    }),
    (error) => {
      assert.equal(error.status, 503);
      assert.equal(error.errorCode, "dependency_unavailable");
      assert.equal(error.action, "pause");
      assert.match(error.message, /dependency unavailable/i);
      assert.ok(!Number.isNaN(Date.parse(error.timestampUtc)));
      return true;
    },
  );
});

test("Story 3.9 control-actions returns explicit machine-readable unauthorized errors", async () => {
  const { invokeEmergencyControlAction } = loadCompiledModule();

  await assert.rejects(
    invokeEmergencyControlAction({
      baseUrl: "http://127.0.0.1:8080",
      action: "pause",
      fetchImpl: async () => ({
        ok: false,
        status: 403,
        json: async () => ({
          error_code: "emergency_control_forbidden",
          message: "operator is not authorized for pause",
          action: "pause",
          correlation_id: "corr-forbidden-001",
          timestamp_utc: "2026-04-06T12:00:05.000Z",
        }),
      }),
    }),
    (error) => {
      assert.equal(error.status, 403);
      assert.equal(error.errorCode, "emergency_control_forbidden");
      assert.equal(error.action, "pause");
      assert.equal(error.correlationId, "corr-forbidden-001");
      return true;
    },
  );
});

test("Story 3.9 control-actions rejects readiness payload with malformed evaluated_at_utc evidence", async () => {
  const { invokeRecoveryReadinessEvaluation } = loadCompiledModule();

  await assert.rejects(
    invokeRecoveryReadinessEvaluation({
      baseUrl: "http://127.0.0.1:8080",
      profileKey: "default",
      reconciliationRunId: "recon-1",
      approvedChecksum:
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      signoffIntent: "approve controlled recovery",
      fetchImpl: async () => ({
        ok: true,
        status: 200,
        json: async () =>
          acceptedReadinessPayload({
            evaluated_at_utc: "definitely-not-a-timestamp",
          }),
      }),
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "recovery_contract_mismatch");
      assert.match(error.message, /evaluated_at_utc/i);
      return true;
    },
  );
});

test("Story 3.9 control-actions rejects readiness payload with malformed timestamp_utc evidence", async () => {
  const { invokeRecoveryReadinessEvaluation } = loadCompiledModule();

  await assert.rejects(
    invokeRecoveryReadinessEvaluation({
      baseUrl: "http://127.0.0.1:8080",
      profileKey: "default",
      reconciliationRunId: "recon-1",
      approvedChecksum:
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      signoffIntent: "approve controlled recovery",
      fetchImpl: async () => ({
        ok: true,
        status: 200,
        json: async () =>
          acceptedReadinessPayload({
            timestamp_utc: "definitely-not-a-timestamp",
          }),
      }),
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "recovery_contract_mismatch");
      assert.match(error.message, /timestamp_utc/i);
      return true;
    },
  );
});

test("Story 3.9 control-actions rejects resume payload with malformed verification_timestamp_utc evidence", async () => {
  const { invokeRecoveryResume } = loadCompiledModule();

  await assert.rejects(
    invokeRecoveryResume({
      baseUrl: "http://127.0.0.1:8080",
      runId: "recovery::gate::default::corr-1::20260406120000",
      resumedAtUtc: "2026-04-06T12:00:05.000Z",
      fetchImpl: async () => ({
        ok: true,
        status: 202,
        json: async () => ({
          status: "accepted",
          action: "recovery_resume_execute",
          run_id: "recovery::gate::default::corr-1::20260406120000",
          readiness_status: "approved",
          reason_code: "recovery_resume_approved",
          verification_reason_code: "recovery_resume_approved",
          actor_id: "ops-1",
          actor_role: "operational_control",
          correlation_id: "corr-1",
          resumed_at_utc: "2026-04-06T12:00:05.000Z",
          verification_timestamp_utc: "not-an-iso-timestamp",
          timestamp_utc: "2026-04-06T12:00:05.000Z",
        }),
      }),
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "recovery_contract_mismatch");
      assert.match(error.message, /verification_timestamp_utc/i);
      return true;
    },
  );
});

test("Story 3.9 control-actions preserves explicit outcome/timestamp evidence on valid emergency response", async () => {
  const { invokeEmergencyControlAction } = loadCompiledModule();

  const result = await invokeEmergencyControlAction({
    baseUrl: "http://127.0.0.1:8080",
    action: "pause",
    fetchImpl: async () => ({
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
        timestamp_utc: "2026-04-06T12:00:05.000Z",
      }),
    }),
  });

  assert.equal(result.actionId, "action::pause-001");
  assert.equal(result.resultingMode, "paused");
  assert.equal(result.reasonCode, "emergency_control_pause_activated");
  assert.equal(result.correlationId, "corr-001");
  assert.equal(result.timestampUtc, "2026-04-06T12:00:05.000Z");
});
