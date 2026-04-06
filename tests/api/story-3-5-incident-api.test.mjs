import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { rmSync, mkdtempSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const PROJECT_ROOT = resolve(".");
const OPERATOR_CONSOLE_FILTER = "operator-console";
const INCIDENT_SOURCE_PATH = "src/lib/incidents/forensics.ts";

let compiledOutputDir;
let incidentModule;

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
  if (incidentModule) {
    return incidentModule;
  }

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-3-5-incident-api-"));
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
      INCIDENT_SOURCE_PATH,
    ],
    { cwd: PROJECT_ROOT, encoding: "utf8" },
  );
  assert.equal(compileResult.status, 0, compileResult.stderr || compileResult.stdout);

  const compiledPath = findFileRecursively(compiledOutputDir, "forensics.js");
  assert.ok(compiledPath, "expected compiled forensics.js output");

  const require = createRequire(import.meta.url);
  incidentModule = require(compiledPath);
  return incidentModule;
}

test.after(() => {
  if (compiledOutputDir) {
    rmSync(compiledOutputDir, { recursive: true, force: true });
  }
});

test("Story 3.5 incident API maps canonical endpoint and causal timeline payload", async () => {
  const { queryIncidentForensics } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 200,
      json: async () => ({
        status: "accepted",
        action: "incident_query",
        actor_id: "ops-1",
        role: "operational_control",
        correlation_id: "corr-incident-api-001",
        timestamp_utc: "2026-04-06T16:00:00.000Z",
        start_inclusive_utc: "2026-04-06T14:00:00.000Z",
        end_exclusive_utc: "2026-04-06T16:00:00.000Z",
        source: "incident.forensics.v1",
        reason_code: "incident_ready",
        data_state: "ready",
        severity: "warning",
        query_latency_ms: 188,
        p95_latency_target_ms: 5000,
        recommended_next_action: "Inspect order/fill divergence before escalation.",
        filters: {
          market_id: "market-btc-election",
          order_id: "order-101",
        },
        causal_flow: {
          trigger: "Mismatch signal detected.",
          context: "Order lifecycle diverged.",
          action: "Reduce-only recommended.",
          verification: "PnL drag confirmed.",
        },
        events: [
          {
            event_id: "incident::order::order-101",
            occurred_at: "2026-04-06T15:57:00.000Z",
            stage: "order",
            source: "reconciliation.diffs.v1",
            reason_code: "reconciliation_non_critical_mismatch",
            correlation_id: "corr-incident-api-001",
            summary: "Order mismatch detected.",
            recommended_next_action: "Inspect venue/internal lifecycle values.",
            severity: "warning",
            market_id: "market-btc-election",
            order_id: "order-101",
            run_id: "run-incident-001",
          },
        ],
      }),
    };
  };

  const result = await queryIncidentForensics({
    baseUrl: "http://127.0.0.1:8080",
    marketId: "market-btc-election",
    orderId: "order-101",
    startTs: "2026-04-06T14:00:00.000Z",
    endTs: "2026-04-06T16:00:00.000Z",
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/incidents/forensics?start_ts=2026-04-06T14%3A00%3A00.000Z&end_ts=2026-04-06T16%3A00%3A00.000Z&market_id=market-btc-election&order_id=order-101",
  );
  assert.equal(requests[0].init.method, "GET");
  assert.equal(result.status, "accepted");
  assert.equal(result.action, "incident_query");
  assert.equal(result.dataState, "ready");
  assert.equal(result.severity, "warning");
  assert.equal(result.events[0].stage, "order");
  assert.equal(result.events[0].reasonCode, "reconciliation_non_critical_mismatch");
});

test("Story 3.5 incident API preserves degraded severity for successful responses", async () => {
  const { queryIncidentForensics } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      status: "accepted",
      action: "incident_query",
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-incident-api-degraded",
      timestamp_utc: "2026-04-06T16:00:00.000Z",
      start_inclusive_utc: "2026-04-06T14:00:00.000Z",
      end_exclusive_utc: "2026-04-06T16:00:00.000Z",
      source: "incident.forensics.v1",
      reason_code: "incident_ready",
      data_state: "ready",
      severity: "degraded",
      query_latency_ms: 244,
      p95_latency_target_ms: 5000,
      recommended_next_action:
        "Keep containment controls active and validate dependency freshness.",
      filters: {
        market_id: "market-btc-election",
      },
      causal_flow: {
        trigger: "Mismatch signal detected.",
        context: "Order lifecycle diverged.",
        action: "Containment active.",
        verification: "Dependency freshness pending.",
      },
      events: [
        {
          event_id: "incident::order::order-101",
          occurred_at: "2026-04-06T15:57:00.000Z",
          stage: "order",
          source: "reconciliation.diffs.v1",
          reason_code: "reconciliation_non_critical_mismatch",
          correlation_id: "corr-incident-api-degraded",
          summary: "Order mismatch detected.",
          recommended_next_action: "Inspect venue/internal lifecycle values.",
          severity: "degraded",
          market_id: "market-btc-election",
          order_id: "order-101",
          run_id: "run-incident-001",
        },
      ],
    }),
  });

  const result = await queryIncidentForensics({
    baseUrl: "http://127.0.0.1:8080",
    startTs: "2026-04-06T14:00:00.000Z",
    endTs: "2026-04-06T16:00:00.000Z",
    fetchImpl,
  });

  assert.equal(result.severity, "degraded");
  assert.equal(result.events[0].severity, "degraded");
});

test("Story 3.5 incident API surfaces dependency-unavailable machine errors", async () => {
  const { queryIncidentForensics } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: false,
    status: 503,
    json: async () => ({
      error_code: "incident_dependency_unavailable",
      reason_code: "incident_dependency_unavailable",
      message: "incident forensics dependencies unavailable",
      action: "incident_query",
      correlation_id: "corr-incident-503",
      timestamp_utc: "2026-04-06T16:00:01.000Z",
      endpoint: "/control/incidents/forensics",
      source: "control-api.incident-forensics.v1",
      occurred_at: "2026-04-06T16:00:01.000Z",
    }),
  });

  await assert.rejects(
    queryIncidentForensics({
      baseUrl: "http://127.0.0.1:8080",
      startTs: "2026-04-06T14:00:00.000Z",
      endTs: "2026-04-06T16:00:00.000Z",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 503);
      assert.equal(error.errorCode, "incident_dependency_unavailable");
      assert.equal(error.reasonCode, "incident_dependency_unavailable");
      assert.equal(error.action, "incident_query");
      assert.equal(error.correlationId, "corr-incident-503");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );
});

test("Story 3.5 incident API rejects half-open time windows before dispatch", async () => {
  const { queryIncidentForensics } = loadCompiledModule();

  await assert.rejects(
    queryIncidentForensics({
      baseUrl: "http://127.0.0.1:8080",
      startTs: "2026-04-06T14:00:00.000Z",
      fetchImpl: async () => {
        throw new Error("fetch should not run for half-open windows");
      },
    }),
    (error) => {
      assert.equal(error.status, 400);
      assert.equal(error.errorCode, "incident_invalid_payload");
      assert.equal(error.fieldErrors[0].field, "start_ts");
      assert.equal(error.fieldErrors[1].field, "end_ts");
      return true;
    },
  );
});

test("Story 3.5 incident API rejects malformed success payloads with contract mismatch", async () => {
  const { queryIncidentForensics } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      status: "accepted",
      action: "incident_query",
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-incident-contract",
      timestamp_utc: "2026-04-06T16:00:00.000Z",
      start_inclusive_utc: "2026-04-06T14:00:00.000Z",
      end_exclusive_utc: "2026-04-06T16:00:00.000Z",
      source: "incident.forensics.v1",
      reason_code: "incident_ready",
      data_state: "ready",
      severity: "warning",
      query_latency_ms: 188,
      p95_latency_target_ms: 5000,
      recommended_next_action: "Inspect order/fill divergence before escalation.",
      filters: {},
      causal_flow: {},
      events: [],
    }),
  });

  await assert.rejects(
    queryIncidentForensics({
      baseUrl: "http://127.0.0.1:8080",
      startTs: "2026-04-06T14:00:00.000Z",
      endTs: "2026-04-06T16:00:00.000Z",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "incident_contract_mismatch");
      assert.match(error.message, /missing required response field/i);
      return true;
    },
  );
});

test("Story 3.5 incident API serializes canonical filter set including alpha and actor identifiers", async () => {
  const { queryIncidentForensics } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 200,
      json: async () => ({
        status: "accepted",
        action: "incident_query",
        actor_id: "ops-1",
        role: "operational_control",
        correlation_id: "corr-incident-all-filters",
        timestamp_utc: "2026-04-06T16:00:00.000Z",
        start_inclusive_utc: "2026-04-06T14:00:00.000Z",
        end_exclusive_utc: "2026-04-06T16:00:00.000Z",
        source: "incident.forensics.v1",
        reason_code: "incident_ready",
        data_state: "ready",
        severity: "warning",
        query_latency_ms: 211,
        p95_latency_target_ms: 5000,
        recommended_next_action: "Validate event chronology and reconcile order lifecycle.",
        filters: {
          market_id: "market-btc-election",
          order_id: "order-101",
          alpha_id: "alpha-momentum",
          actor_id: "ops-1",
        },
        causal_flow: {
          trigger: "Mismatch signal detected.",
          context: "Order lifecycle diverged.",
          action: "Containment guidance issued.",
          verification: "PnL divergence remains bounded.",
        },
        events: [
          {
            event_id: "incident::signal::001",
            occurred_at: "2026-04-06T15:57:00.000Z",
            stage: "signal",
            source: "risk.signals.v1",
            reason_code: "incident_signal_detected",
            correlation_id: "corr-incident-all-filters",
            summary: "Signal anomaly detected.",
            recommended_next_action: "Confirm reconciliation context.",
            severity: "warning",
            market_id: "market-btc-election",
            order_id: "order-101",
            alpha_id: "alpha-momentum",
            actor_id: "ops-1",
            run_id: "run-incident-001",
          },
        ],
      }),
    };
  };

  const result = await queryIncidentForensics({
    baseUrl: "http://127.0.0.1:8080",
    marketId: "  MARKET-BTC-ELECTION  ",
    orderId: "  ORDER-101  ",
    alphaId: "  ALPHA-MOMENTUM  ",
    actorId: "  OPS-1  ",
    startTs: "2026-04-06T14:00:00.000Z",
    endTs: "2026-04-06T16:00:00.000Z",
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  const requestUrl = new URL(requests[0].url);
  assert.equal(requestUrl.pathname, "/control/incidents/forensics");
  assert.equal(requestUrl.searchParams.get("market_id"), "market-btc-election");
  assert.equal(requestUrl.searchParams.get("order_id"), "order-101");
  assert.equal(requestUrl.searchParams.get("alpha_id"), "alpha-momentum");
  assert.equal(requestUrl.searchParams.get("actor_id"), "ops-1");
  assert.equal(result.filters.alphaId, "alpha-momentum");
  assert.equal(result.filters.actorId, "ops-1");
});

test("Story 3.5 incident API preserves explicit empty-window responses", async () => {
  const { queryIncidentForensics } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      status: "accepted",
      action: "incident_query",
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-incident-empty-window",
      timestamp_utc: "2026-04-06T16:00:00.000Z",
      start_inclusive_utc: "2026-04-06T14:00:00.000Z",
      end_exclusive_utc: "2026-04-06T16:00:00.000Z",
      source: "incident.forensics.v1",
      reason_code: "incident_no_match",
      data_state: "empty",
      severity: "normal",
      query_latency_ms: 141,
      p95_latency_target_ms: 5000,
      recommended_next_action:
        "Expand time bounds or remove restrictive filters to recover evidence.",
      filters: {
        actor_id: "ops-1",
      },
      causal_flow: {
        trigger: "No matching incident chain in window.",
        context: "Current filter set returned no correlated evidence.",
        action: "Widen search scope.",
        verification: "Re-run query and confirm correlated stages appear.",
      },
      events: [],
    }),
  });

  const result = await queryIncidentForensics({
    baseUrl: "http://127.0.0.1:8080",
    actorId: "ops-1",
    startTs: "2026-04-06T14:00:00.000Z",
    endTs: "2026-04-06T16:00:00.000Z",
    fetchImpl,
  });

  assert.equal(result.dataState, "empty");
  assert.equal(result.severity, "normal");
  assert.equal(result.events.length, 0);
  assert.match(result.recommendedNextAction, /expand time bounds/i);
});

test("Story 3.5 incident API surfaces unauthorized machine errors", async () => {
  const { queryIncidentForensics } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: false,
    status: 403,
    json: async () => ({
      error_code: "incident_unauthorized",
      reason_code: "incident_unauthorized",
      message: "caller is not authorized to read incident forensics data",
      action: "incident_query",
      correlation_id: "corr-incident-403",
      timestamp_utc: "2026-04-06T16:00:02.000Z",
      endpoint: "/control/incidents/forensics",
      source: "control-api.incident-forensics.v1",
      occurred_at: "2026-04-06T16:00:02.000Z",
    }),
  });

  await assert.rejects(
    queryIncidentForensics({
      baseUrl: "http://127.0.0.1:8080",
      startTs: "2026-04-06T14:00:00.000Z",
      endTs: "2026-04-06T16:00:00.000Z",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 403);
      assert.equal(error.errorCode, "incident_unauthorized");
      assert.equal(error.reasonCode, "incident_unauthorized");
      assert.equal(error.action, "incident_query");
      assert.equal(error.correlationId, "corr-incident-403");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );
});
