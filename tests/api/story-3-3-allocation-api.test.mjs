import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { rmSync, mkdtempSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const PROJECT_ROOT = resolve(".");
const OPERATOR_CONSOLE_FILTER = "operator-console";
const ALLOCATION_POLICY_SOURCE_PATH = "src/lib/portfolio/allocation-policy.ts";

let compiledOutputDir;
let allocationPolicyModule;

function parseIsoTimestamp(value) {
  const parsed = Date.parse(value);
  assert.ok(!Number.isNaN(parsed), `expected ISO-8601 timestamp, received: ${value}`);
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

function loadCompiledModule() {
  if (allocationPolicyModule) {
    return allocationPolicyModule;
  }

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-3-3-allocation-api-"));
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
      ALLOCATION_POLICY_SOURCE_PATH,
    ],
    { cwd: PROJECT_ROOT, encoding: "utf8" },
  );
  assert.equal(compileResult.status, 0, compileResult.stderr || compileResult.stdout);

  const compiledPath = findFileRecursively(compiledOutputDir, "allocation-policy.js");
  assert.ok(compiledPath, "expected compiled allocation-policy.js output");

  const require = createRequire(import.meta.url);
  allocationPolicyModule = require(compiledPath);
  return allocationPolicyModule;
}

test.after(() => {
  if (compiledOutputDir) {
    rmSync(compiledOutputDir, { recursive: true, force: true });
  }
});

test("Story 3.3 allocation API maps policy upsert evidence and canonical endpoint path", async () => {
  const { upsertAllocationPolicy } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 202,
      json: async () => ({
        status: "pending",
        policy_key: "portfolio-default",
        version: 2,
        approval_status: "pending",
        actor_id: "ops-1",
        role: "operational_control",
        reason_code: "allocation_policy_pending_approval",
        correlation_id: "corr-allocation-api-001",
        timestamp_utc: "2026-04-06T12:00:00.000Z",
      }),
    };
  };

  const result = await upsertAllocationPolicy({
    baseUrl: "http://127.0.0.1:8080",
    policyKey: "portfolio-default",
    version: 2,
    portfolioScopeId: "portfolio::default",
    targetExposurePctNav: 32.5,
    targetRelativeAlphaWeight: 1.25,
    exposureDriftThresholdPct: 10,
    relativeAlphaDriftThresholdPct: 15,
    advancedParameters: { mode: "balanced" },
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/allocation-policies/portfolio-default",
  );
  assert.equal(requests[0].init.method, "POST");
  assert.match(requests[0].init.body, /"portfolio_scope_id":"portfolio::default"/);
  assert.match(requests[0].init.body, /"target_exposure_pct_nav":32.5/);
  assert.equal(result.status, "pending");
  assert.equal(result.policyKey, "portfolio-default");
  assert.equal(result.reasonCode, "allocation_policy_pending_approval");
  assert.equal(result.correlationId, "corr-allocation-api-001");
});

test("Story 3.3 rebalance evaluate API preserves machine-readable dependency failures", async () => {
  const { evaluateRebalanceRecommendation } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: false,
    status: 503,
    json: async () => ({
      error_code: "rebalance_persistence_unavailable",
      message: "allocation persistence unavailable",
      action: "rebalance_recommendation_evaluate",
      correlation_id: "corr-rebalance-api-503",
      timestamp_utc: "2026-04-06T12:00:01.000Z",
      endpoint: "/control/rebalance",
    }),
  });

  await assert.rejects(
    evaluateRebalanceRecommendation({
      baseUrl: "http://127.0.0.1:8080",
      policyKey: "portfolio-default",
      exposureDriftPct: 18,
      relativeAlphaDriftPct: 22,
      requireExecution: true,
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 503);
      assert.equal(error.errorCode, "rebalance_persistence_unavailable");
      assert.equal(error.action, "rebalance_recommendation_evaluate");
      assert.equal(error.correlationId, "corr-rebalance-api-503");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );
});

test("Story 3.3 pending recommendation query uses canonical endpoint and response mapping", async () => {
  const { listPendingRebalanceRecommendations } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 200,
      json: async () => ({
        status: "accepted",
        action: "rebalance_pending_query",
        actor_id: "ops-1",
        role: "operational_control",
        correlation_id: "corr-pending-api-001",
        timestamp_utc: "2026-04-06T12:00:02.000Z",
        pending_recommendations: [
          {
            recommendation_id: "reco::portfolio-default::pending",
            policy_key: "portfolio-default",
            policy_version: 3,
            recommendation_status: "pending_approval",
            approval_status: "pending",
            action_type: "execute",
            rationale: "Critical drift exceeded threshold.",
            recommended_next_action: "Complete dual approval and execute recommendation.",
            actor_id: "ops-1",
            reason_code: "rebalance_approval_required",
            correlation_id: "corr-pending-api-001",
            created_at_utc: "2026-04-06T12:00:02.000Z",
            updated_at_utc: "2026-04-06T12:00:02.000Z",
          },
        ],
      }),
    };
  };

  const result = await listPendingRebalanceRecommendations({
    baseUrl: "http://127.0.0.1:8080",
    policyKey: "portfolio-default",
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/rebalance/recommendations/pending?policy_key=portfolio-default",
  );
  assert.equal(requests[0].init.method, "GET");
  assert.equal(result.status, "accepted");
  assert.equal(result.action, "rebalance_pending_query");
  assert.equal(result.pendingRecommendations[0].recommendationStatus, "pending_approval");
});

test("Story 3.3 execute API uses recommendation execute endpoint and returns execution evidence", async () => {
  const { executeRebalanceRecommendation } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 202,
      json: async () => ({
        status: "accepted",
        recommendation_id: "reco::portfolio-default::exec",
        policy_key: "portfolio-default",
        policy_version: 4,
        recommendation_status: "executed",
        approval_status: "approved",
        action_type: "execute",
        rationale: "Execution completed after approval.",
        recommended_next_action: "Monitor post-execution drift.",
        actor_id: "admin-1",
        role: "administrative_actions",
        reason_code: "rebalance_recommendation_executed",
        correlation_id: "corr-execute-api-001",
        created_at_utc: "2026-04-06T12:00:03.000Z",
        timestamp_utc: "2026-04-06T12:00:04.000Z",
        approval_reference: "apr-rebalance-001",
      }),
    };
  };

  const result = await executeRebalanceRecommendation({
    baseUrl: "http://127.0.0.1:8080",
    recommendationId: "reco::portfolio-default::exec",
    approvalRequestId: "apr-req-001",
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/rebalance/recommendations/reco%3A%3Aportfolio-default%3A%3Aexec/execute",
  );
  assert.equal(requests[0].init.method, "POST");
  assert.match(requests[0].init.body, /"approval_request_id":"apr-req-001"/);
  assert.doesNotMatch(requests[0].init.body, /"approval_reference"/);
  assert.equal(result.status, "accepted");
  assert.equal(result.recommendationStatus, "executed");
  assert.equal(result.approvalStatus, "approved");
});

test("Story 3.3 allocation API preserves 400 validation evidence with field-level diagnostics", async () => {
  const { upsertAllocationPolicy } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: false,
      status: 400,
      json: async () => ({
        error_code: "allocation_policy_validation_failed",
        message: "allocation policy payload failed validation",
        action: "allocation_policy_update",
        correlation_id: "corr-allocation-api-400",
        timestamp_utc: "2026-04-06T12:00:05.000Z",
        endpoint: "/control/allocation-policies/portfolio-default",
        field_errors: [
          {
            field: "target_exposure_pct_nav",
            code: "range_violation",
            message: "must be between 0 and 100",
          },
          {
            field: "version",
            code: "non_positive",
            message: "must be a positive integer",
          },
        ],
      }),
    };
  };

  await assert.rejects(
    upsertAllocationPolicy({
      baseUrl: "http://127.0.0.1:8080",
      policyKey: "portfolio-default",
      version: 0,
      portfolioScopeId: "portfolio::default",
      targetExposurePctNav: 120,
      targetRelativeAlphaWeight: 1.25,
      exposureDriftThresholdPct: 10,
      relativeAlphaDriftThresholdPct: 15,
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 400);
      assert.equal(error.errorCode, "allocation_policy_validation_failed");
      assert.equal(error.action, "allocation_policy_update");
      assert.equal(error.correlationId, "corr-allocation-api-400");
      assert.equal(error.fieldErrors.length, 2);
      assert.equal(error.fieldErrors[0].field, "target_exposure_pct_nav");
      assert.equal(error.fieldErrors[1].code, "non_positive");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/allocation-policies/portfolio-default",
  );
  assert.equal(requests[0].init.method, "POST");
});

test("Story 3.3 pending recommendation query surfaces canonical 404 machine errors", async () => {
  const { listPendingRebalanceRecommendations } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: false,
      status: 404,
      json: async () => ({
        error_code: "rebalance_policy_not_found",
        message: "policy key not found for pending query",
        action: "rebalance_pending_query",
        correlation_id: "corr-pending-api-404",
        timestamp_utc: "2026-04-06T12:00:06.000Z",
        endpoint:
          "/control/rebalance/recommendations/pending?policy_key=portfolio-missing",
      }),
    };
  };

  await assert.rejects(
    listPendingRebalanceRecommendations({
      baseUrl: "http://127.0.0.1:8080",
      policyKey: "portfolio-missing",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 404);
      assert.equal(error.errorCode, "rebalance_policy_not_found");
      assert.equal(error.action, "rebalance_pending_query");
      assert.equal(error.correlationId, "corr-pending-api-404");
      assert.equal(
        error.endpoint,
        "/control/rebalance/recommendations/pending?policy_key=portfolio-missing",
      );
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/rebalance/recommendations/pending?policy_key=portfolio-missing",
  );
  assert.equal(requests[0].init.method, "GET");
});

test("Story 3.3 execute API wraps transport failures as explicit 500 machine errors", async () => {
  const { executeRebalanceRecommendation } = loadCompiledModule();

  await assert.rejects(
    executeRebalanceRecommendation({
      baseUrl: "http://127.0.0.1:8080",
      recommendationId: "reco::transport-error",
      fetchImpl: async () => {
        throw new Error("socket hang up");
      },
    }),
    (error) => {
      assert.equal(error.status, 500);
      assert.equal(error.errorCode, "allocation_policy_request_failed");
      assert.equal(error.action, "rebalance_recommendation_execute");
      assert.equal(
        error.endpoint,
        "/control/rebalance/recommendations/reco%3A%3Atransport-error/execute",
      );
      assert.match(error.message, /socket hang up/);
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );
});
