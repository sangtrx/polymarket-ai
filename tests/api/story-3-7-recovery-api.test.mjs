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

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-3-7-recovery-api-"));
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

function acceptedReadinessPayload({
  readinessStatus = "approved",
  reasonCode = "recovery_resume_approved",
  failingGateCodes = [],
  gateOutcomes,
  resumedAtUtc,
}) {
  return {
    status: "accepted",
    action: "recovery_readiness_evaluate",
    run_id: "recovery::gate::default::corr-1::20260406120000",
    readiness_status: readinessStatus,
    reason_code: reasonCode,
    profile_key: "default",
    actor_id: "ops-1",
    actor_role: "operational_control",
    correlation_id: "corr-1",
    requested_at_utc: "2026-04-06T12:00:00.000Z",
    evaluated_at_utc: "2026-04-06T12:00:01.000Z",
    resumed_at_utc: resumedAtUtc,
    reconciliation_run_id: "recon-1",
    approved_checksum:
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    computed_checksum:
      readinessStatus === "approved"
        ? "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        : "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    failing_gate_codes: failingGateCodes,
    gate_outcomes:
      gateOutcomes ??
      [
        {
          gate: "freshness",
          passed: true,
          reason_code: "recovery_freshness_pass",
          trigger: "freshness <= 30s",
          context: "freshness_age_seconds=10",
          action: "allow freshness gate",
          verification: "freshness evidence verified",
        },
        {
          gate: "reconciliation",
          passed: true,
          reason_code: "recovery_reconciliation_pass",
          trigger: "reconciliation mismatch < 0.1%",
          context: "mismatch_rate=0.02%",
          action: "allow reconciliation gate",
          verification: "reconciliation evidence verified",
        },
        {
          gate: "risk_checksum",
          passed: true,
          reason_code: "recovery_checksum_match",
          trigger: "exact checksum equality",
          context: "approved checksum equals computed checksum",
          action: "allow checksum gate",
          verification: "checksum evidence verified",
        },
        {
          gate: "operator_signoff",
          passed: true,
          reason_code: "recovery_signoff_recorded",
          trigger: "explicit signoff recorded",
          context: "actor and intent present",
          action: "allow signoff gate",
          verification: "signoff evidence verified",
        },
      ],
    recommended_next_action:
      readinessStatus === "approved"
        ? "Execute controlled recovery resume."
        : "Keep containment active and resolve failed gates.",
    audit_reference: "arb-2026-0007",
    timestamp_utc: "2026-04-06T12:00:01.000Z",
  };
}

test("Story 3.7 readiness API maps approved response payload with gate evidence", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeRecoveryReadinessEvaluation } = controlActionsModule;

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 200,
      json: async () => acceptedReadinessPayload({}),
    };
  };

  const result = await invokeRecoveryReadinessEvaluation({
    baseUrl: "http://127.0.0.1:8080",
    profileKey: "default",
    reconciliationRunId: "recon-1",
    approvedChecksum:
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    signoffIntent: "approve controlled recovery",
    auditReference: "arb-2026-0007",
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/recovery/readiness/evaluate",
  );
  assert.equal(requests[0].init.method, "POST");
  assert.match(requests[0].init.body, /"profile_key":"default"/);
  assert.equal(result.readinessStatus, "approved");
  assert.equal(result.reasonCode, "recovery_resume_approved");
  assert.equal(result.gateOutcomes.length, 4);
  parseIsoTimestamp(result.evaluatedAtUtc);
});

test("Story 3.7 readiness API maps blocked responses with failing gate guidance", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeRecoveryReadinessEvaluation } = controlActionsModule;

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () =>
      acceptedReadinessPayload({
        readinessStatus: "blocked",
        reasonCode: "recovery_resume_blocked",
        failingGateCodes: [
          "recovery_freshness_stale",
          "recovery_reconciliation_mismatch",
        ],
        gateOutcomes: [
          {
            gate: "freshness",
            passed: false,
            reason_code: "recovery_freshness_stale",
            trigger: "freshness <= 30s",
            context: "freshness_age_seconds=45",
            action: "restore freshness telemetry before resuming",
            verification: "freshness evidence refreshed",
          },
          {
            gate: "reconciliation",
            passed: false,
            reason_code: "recovery_reconciliation_mismatch",
            trigger: "reconciliation mismatch < 0.1%",
            context: "mismatch_rate=0.10%",
            action: "rerun reconciliation with clean dependencies",
            verification: "mismatch below boundary on rerun",
          },
        ],
      }),
  });

  const result = await invokeRecoveryReadinessEvaluation({
    baseUrl: "http://127.0.0.1:8080",
    profileKey: "default",
    reconciliationRunId: "recon-1",
    approvedChecksum:
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    signoffIntent: "approve controlled recovery",
    fetchImpl,
  });

  assert.equal(result.readinessStatus, "blocked");
  assert.equal(result.reasonCode, "recovery_resume_blocked");
  assert.deepEqual(result.failingGateCodes, [
    "recovery_freshness_stale",
    "recovery_reconciliation_mismatch",
  ]);
  assert.equal(result.gateOutcomes[0].trigger, "freshness <= 30s");
  assert.equal(result.gateOutcomes[0].context, "freshness_age_seconds=45");
  assert.equal(
    result.gateOutcomes[0].action,
    "restore freshness telemetry before resuming",
  );
  assert.equal(
    result.gateOutcomes[0].verification,
    "freshness evidence refreshed",
  );
});

test("Story 3.7 resume API maps verification envelope fields deterministically", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeRecoveryResume } = controlActionsModule;

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
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
        verification_timestamp_utc: "2026-04-06T12:00:05.000Z",
        timestamp_utc: "2026-04-06T12:00:01.000Z",
        audit_reference: "arb-2026-0007",
      }),
    };
  };

  const result = await invokeRecoveryResume({
    baseUrl: "http://127.0.0.1:8080",
    runId: "recovery::gate::default::corr-1::20260406120000",
    resumedAtUtc: "2026-04-06T12:00:05.000Z",
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(requests[0].url, "http://127.0.0.1:8080/control/recovery/resume");
  assert.equal(requests[0].init.method, "POST");
  assert.equal(result.readinessStatus, "approved");
  assert.equal(result.verificationReasonCode, "recovery_resume_approved");
  parseIsoTimestamp(result.verificationTimestampUtc);
});

test("Story 3.7 workflow timing evidence supports NFR6 10-minute recovery instrumentation", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeRecoveryReadinessEvaluation, invokeRecoveryResume } = controlActionsModule;

  const fetchImpl = async (url) => {
    if (url.endsWith("/control/recovery/readiness/evaluate")) {
      return {
        ok: true,
        status: 200,
        json: async () =>
          acceptedReadinessPayload({
            readinessStatus: "approved",
            reasonCode: "recovery_resume_approved",
          }),
      };
    }

    if (url.endsWith("/control/recovery/resume")) {
      return {
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
          resumed_at_utc: "2026-04-06T12:09:30.000Z",
          verification_timestamp_utc: "2026-04-06T12:09:30.000Z",
          timestamp_utc: "2026-04-06T12:09:30.000Z",
        }),
      };
    }

    throw new Error(`unexpected endpoint: ${url}`);
  };

  const readiness = await invokeRecoveryReadinessEvaluation({
    baseUrl: "http://127.0.0.1:8080",
    profileKey: "default",
    reconciliationRunId: "recon-1",
    approvedChecksum:
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    signoffIntent: "approve controlled recovery",
    fetchImpl,
  });
  const resume = await invokeRecoveryResume({
    baseUrl: "http://127.0.0.1:8080",
    runId: readiness.runId,
    resumedAtUtc: "2026-04-06T12:09:30.000Z",
    fetchImpl,
  });

  const workflowDurationMs =
    parseIsoTimestamp(resume.verificationTimestampUtc) -
    parseIsoTimestamp(readiness.requestedAtUtc);
  assert.ok(
    workflowDurationMs <= 10 * 60 * 1000,
    `expected workflow duration <= 10 minutes, received ${workflowDurationMs}ms`,
  );
});

test("Story 3.7 run query API supports correlation selector endpoint", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { queryRecoveryGateRun } = controlActionsModule;

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 200,
      json: async () =>
        acceptedReadinessPayload({
          readinessStatus: "blocked",
          reasonCode: "recovery_resume_blocked",
          failingGateCodes: ["recovery_checksum_mismatch"],
        }),
    };
  };

  const result = await queryRecoveryGateRun({
    baseUrl: "http://127.0.0.1:8080",
    correlationId: "corr-1",
    fetchImpl,
  });

  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/recovery/runs?correlation_id=corr-1",
  );
  assert.equal(requests[0].init.method, "GET");
  assert.equal(result.readinessStatus, "blocked");
});

test("Story 3.7 run query API supports run-id selector endpoint", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { queryRecoveryGateRun } = controlActionsModule;

  const runId = "recovery::gate::default::corr-2::20260406120500";
  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 200,
      json: async () => ({
        ...acceptedReadinessPayload({
          readinessStatus: "approved",
          reasonCode: "recovery_resume_approved",
        }),
        run_id: runId,
        correlation_id: "corr-2",
      }),
    };
  };

  const result = await queryRecoveryGateRun({
    baseUrl: "http://127.0.0.1:8080",
    runId,
    fetchImpl,
  });

  assert.equal(
    requests[0].url,
    `http://127.0.0.1:8080/control/recovery/runs/${encodeURIComponent(runId)}`,
  );
  assert.equal(requests[0].init.method, "GET");
  assert.equal(result.runId, runId);
  assert.equal(result.readinessStatus, "approved");
});

test("Story 3.7 run query API rejects missing selectors before network request", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { queryRecoveryGateRun } = controlActionsModule;

  await assert.rejects(
    queryRecoveryGateRun({
      baseUrl: "http://127.0.0.1:8080",
      fetchImpl: async () => {
        throw new Error("fetch should not be called for missing query selectors");
      },
    }),
    (error) => {
      assert.equal(error.status, 400);
      assert.equal(error.errorCode, "recovery_query_missing_selector");
      assert.equal(error.action, "recovery_readiness_query");
      return true;
    },
  );
});

test("Story 3.7 resume API surfaces unauthorized machine errors", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeRecoveryResume } = controlActionsModule;

  const fetchImpl = async () => ({
    ok: false,
    status: 401,
    json: async () => ({
      error_code: "recovery_unauthorized",
      reason_code: "recovery_unauthorized",
      message: "operator role is required for controlled recovery resume",
      action: "recovery_resume_execute",
      correlation_id: "corr-recovery-401",
      timestamp_utc: "2026-04-06T12:00:05.000Z",
      endpoint: "/control/recovery/resume",
    }),
  });

  await assert.rejects(
    invokeRecoveryResume({
      baseUrl: "http://127.0.0.1:8080",
      runId: "recovery::gate::default::corr-1::20260406120000",
      resumedAtUtc: "2026-04-06T12:00:05.000Z",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 401);
      assert.equal(error.errorCode, "recovery_unauthorized");
      assert.equal(error.action, "recovery_resume_execute");
      assert.equal(error.correlationId, "corr-recovery-401");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );
});

test("Story 3.7 resume API rejects malformed success payload missing verification timestamp", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeRecoveryResume } = controlActionsModule;

  const fetchImpl = async () => ({
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
      timestamp_utc: "2026-04-06T12:00:05.000Z",
    }),
  });

  await assert.rejects(
    invokeRecoveryResume({
      baseUrl: "http://127.0.0.1:8080",
      runId: "recovery::gate::default::corr-1::20260406120000",
      resumedAtUtc: "2026-04-06T12:00:05.000Z",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "emergency_control_contract_mismatch");
      assert.match(error.message, /verification_timestamp_utc/i);
      return true;
    },
  );
});

test("Story 3.7 readiness preflight rejects malformed checksums before network request", async () => {
  const { controlActionsModule } = loadCompiledModules();
  const { invokeRecoveryReadinessEvaluation } = controlActionsModule;

  await assert.rejects(
    invokeRecoveryReadinessEvaluation({
      baseUrl: "http://127.0.0.1:8080",
      profileKey: "default",
      reconciliationRunId: "recon-1",
      approvedChecksum: "not-a-checksum",
      signoffIntent: "approve controlled recovery",
      fetchImpl: async () => {
        throw new Error("fetch should not be called for malformed checksums");
      },
    }),
    (error) => {
      assert.equal(error.status, 400);
      assert.equal(error.errorCode, "recovery_invalid_checksum");
      return true;
    },
  );
});

test("Story 3.7 posture transitions apply blocked and completed recovery evidence states", () => {
  const { postureModule } = loadCompiledModules();
  const { resolveRiskPostureViewModel, withRecoveryReadinessEvidence, withRecoveryResumeEvidence } =
    postureModule;

  const baseline = resolveRiskPostureViewModel(
    { riskState: "locked-safe" },
    {
      dataState: "ready",
      freshness: {
        lastUpdatedIso: "2026-04-06T12:00:00.000Z",
        source: "dashboard.read-model.shell",
        isStale: false,
      },
      p95TargetMs: 2000,
    },
  );

  const blocked = withRecoveryReadinessEvidence(
    baseline,
    {
      status: "accepted",
      action: "recovery_readiness_evaluate",
      runId: "recovery::gate::default::corr-1::20260406120000",
      readinessStatus: "blocked",
      reasonCode: "recovery_resume_blocked",
      profileKey: "default",
      actorId: "ops-1",
      actorRole: "operational_control",
      correlationId: "corr-1",
      requestedAtUtc: "2026-04-06T12:00:00.000Z",
      evaluatedAtUtc: "2026-04-06T12:00:01.000Z",
      failingGateCodes: ["recovery_checksum_mismatch"],
      gateOutcomes: [
        {
          gate: "risk_checksum",
          passed: false,
          reasonCode: "recovery_checksum_mismatch",
          trigger: "exact checksum equality",
          context: "approved != computed",
          action: "refresh approved checksum",
          verification: "checksum evidence rerun",
        },
      ],
      recommendedNextAction: "refresh approved checksum",
      timestampUtc: "2026-04-06T12:00:01.000Z",
      reconciliationRunId: "recon-1",
      approvedChecksum:
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      computedChecksum:
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    },
  );

  assert.equal(blocked.resumeState, "blocked-with-reasons");
  assert.equal(blocked.resumeFailureDetails.length, 1);
  assert.equal(blocked.resumeFailureDetails[0].gate, "risk_checksum");

  const completed = withRecoveryResumeEvidence(blocked, {
    status: "accepted",
    action: "recovery_resume_execute",
    runId: "recovery::gate::default::corr-1::20260406120000",
    readinessStatus: "approved",
    reasonCode: "recovery_resume_approved",
    verificationReasonCode: "recovery_resume_approved",
    actorId: "ops-1",
    actorRole: "operational_control",
    correlationId: "corr-1",
    resumedAtUtc: "2026-04-06T12:00:05.000Z",
    verificationTimestampUtc: "2026-04-06T12:00:05.000Z",
    timestampUtc: "2026-04-06T12:00:05.000Z",
    auditReference: "arb-2026-0007",
  });

  assert.equal(completed.resumeState, "completed");
  assert.equal(completed.posture, "normal");
  assert.equal(completed.evidence.reasonCode, "recovery_resume_approved");
});
