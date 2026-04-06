import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { rmSync, mkdtempSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const PROJECT_ROOT = resolve(".");
const OPERATOR_CONSOLE_FILTER = "operator-console";
const ATTRIBUTION_SOURCE_PATH = "src/lib/portfolio/attribution.ts";

let compiledOutputDir;
let attributionModule;

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
  if (attributionModule) {
    return attributionModule;
  }

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-3-4-attribution-api-"));
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
      ATTRIBUTION_SOURCE_PATH,
    ],
    { cwd: PROJECT_ROOT, encoding: "utf8" },
  );
  assert.equal(compileResult.status, 0, compileResult.stderr || compileResult.stdout);

  const compiledPath = findFileRecursively(compiledOutputDir, "attribution.js");
  assert.ok(compiledPath, "expected compiled attribution.js output");

  const require = createRequire(import.meta.url);
  attributionModule = require(compiledPath);
  return attributionModule;
}

test.after(() => {
  if (compiledOutputDir) {
    rmSync(compiledOutputDir, { recursive: true, force: true });
  }
});

test("Story 3.4 attribution API maps canonical endpoint and metadata-rich rows", async () => {
  const { queryCostAwareAttribution } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 200,
      json: async () => ({
        status: "accepted",
        action: "attribution_query",
        actor_id: "ops-1",
        role: "operational_control",
        correlation_id: "corr-attribution-api-001",
        timestamp_utc: "2026-04-06T15:00:00.000Z",
        period: "24h",
        start_inclusive_utc: "2026-04-05T15:00:00.000Z",
        end_exclusive_utc: "2026-04-06T15:00:00.000Z",
        as_of_utc: "2026-04-06T15:00:00.000Z",
        source: "reconciliation.exposure.v1",
        reason_code: "attribution_ready",
        data_state: "ready",
        recommended_next_action: "Inspect top net contributors first.",
        rows: [
          {
            market_id: "market-btc-election",
            alpha_id: "alpha-momentum",
            period: "24h",
            period_start_utc: "2026-04-05T15:00:00.000Z",
            period_end_utc: "2026-04-06T15:00:00.000Z",
            realized_pnl_usd: 132.5,
            unrealized_pnl_usd: 24.0,
            gross_pnl_usd: 156.5,
            net_pnl_usd: 152.2,
            fees_usd: 6.8,
            rebates_usd: 1.9,
            incentives_usd: 0.6,
            net_cost_impact_usd: 4.3,
            as_of_utc: "2026-04-06T15:00:00.000Z",
            source: "reconciliation.exposure.v1",
            reason_code: "attribution_ready",
            correlation_id: "corr-attribution-api-001",
            snapshot_id: "snapshot::attribution::btc::001",
            run_id: "run::reconciliation::btc::001",
          },
        ],
      }),
    };
  };

  const result = await queryCostAwareAttribution({
    baseUrl: "http://127.0.0.1:8080",
    period: "24h",
    marketId: "market-btc-election",
    alphaId: "alpha-momentum",
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/portfolio/attribution?period=24h&market_id=market-btc-election&alpha_id=alpha-momentum",
  );
  assert.equal(requests[0].init.method, "GET");
  assert.equal(result.status, "accepted");
  assert.equal(result.dataState, "ready");
  assert.equal(result.rows.length, 1);
  assert.equal(result.rows[0].reasonCode, "attribution_ready");
  assert.equal(result.rows[0].correlationId, "corr-attribution-api-001");
});

test("Story 3.4 attribution API surfaces machine-readable dependency failures", async () => {
  const { queryCostAwareAttribution } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: false,
    status: 503,
    json: async () => ({
      error_code: "attribution_projection_unavailable",
      message: "projection dependency unavailable",
      action: "attribution_query",
      correlation_id: "corr-attribution-503",
      timestamp_utc: "2026-04-06T15:00:01.000Z",
      endpoint: "/control/portfolio/attribution?period=24h",
    }),
  });

  await assert.rejects(
    queryCostAwareAttribution({
      baseUrl: "http://127.0.0.1:8080",
      period: "24h",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 503);
      assert.equal(error.errorCode, "attribution_projection_unavailable");
      assert.equal(error.action, "attribution_query");
      assert.equal(error.correlationId, "corr-attribution-503");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );
});

test("Story 3.4 attribution API surfaces unauthorized failures with canonical query evidence", async () => {
  const { queryCostAwareAttribution } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: false,
      status: 403,
      json: async () => ({
        error_code: "attribution_unauthorized",
        message: "attribution read requires operational_control role",
        action: "attribution_query",
        correlation_id: "corr-attribution-403",
        timestamp_utc: "2026-04-06T15:00:02.000Z",
        endpoint:
          "/control/portfolio/attribution?period=1h&as_of_utc=2026-04-06T15%3A00%3A00.000Z&dependency_state=stale_source",
      }),
    };
  };

  await assert.rejects(
    queryCostAwareAttribution({
      baseUrl: "http://127.0.0.1:8080/",
      period: "1h",
      asOfUtc: "2026-04-06T15:00:00.000Z",
      dependencyState: "stale_source",
      fetchImpl,
    }),
    (error) => {
      assert.equal(requests.length, 1);
      assert.equal(
        requests[0].url,
        "http://127.0.0.1:8080/control/portfolio/attribution?period=1h&as_of_utc=2026-04-06T15%3A00%3A00.000Z&dependency_state=stale_source",
      );
      assert.equal(requests[0].init.method, "GET");
      assert.equal(error.status, 403);
      assert.equal(error.errorCode, "attribution_unauthorized");
      assert.equal(error.action, "attribution_query");
      assert.equal(
        error.endpoint,
        "/control/portfolio/attribution?period=1h&as_of_utc=2026-04-06T15%3A00%3A00.000Z&dependency_state=stale_source",
      );
      assert.equal(error.correlationId, "corr-attribution-403");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );
});

test("Story 3.4 attribution API rejects invalid period filters before request dispatch", async () => {
  const { queryCostAwareAttribution } = loadCompiledModule();

  await assert.rejects(
    queryCostAwareAttribution({
      baseUrl: "http://127.0.0.1:8080",
      period: "7d",
      fetchImpl: async () => {
        throw new Error("fetch should not run for invalid period");
      },
    }),
    (error) => {
      assert.equal(error.status, 400);
      assert.equal(error.errorCode, "attribution_invalid_period");
      assert.equal(error.fieldErrors[0].field, "period");
      return true;
    },
  );
});

test("Story 3.4 attribution API rejects malformed success payloads with explicit contract mismatch", async () => {
  const { queryCostAwareAttribution } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      status: "accepted",
      action: "attribution_query",
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-attribution-contract",
      timestamp_utc: "2026-04-06T15:00:01.000Z",
      period: "24h",
      start_inclusive_utc: "2026-04-05T15:00:00.000Z",
      end_exclusive_utc: "2026-04-06T15:00:00.000Z",
      as_of_utc: "2026-04-06T15:00:00.000Z",
      source: "reconciliation.exposure.v1",
      reason_code: "attribution_ready",
      data_state: "ready",
      recommended_next_action: "Inspect top net contributors first.",
      rows: {},
    }),
  });

  await assert.rejects(
    queryCostAwareAttribution({
      baseUrl: "http://127.0.0.1:8080",
      period: "24h",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "attribution_contract_mismatch");
      assert.match(error.message, /rows must be an array/i);
      return true;
    },
  );
});

test("Story 3.4 attribution API rejects malformed timestamp evidence in success payload", async () => {
  const { queryCostAwareAttribution } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      status: "accepted",
      action: "attribution_query",
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-attribution-timestamp",
      timestamp_utc: "not-a-timestamp",
      period: "24h",
      start_inclusive_utc: "2026-04-05T15:00:00.000Z",
      end_exclusive_utc: "2026-04-06T15:00:00.000Z",
      as_of_utc: "2026-04-06T15:00:00.000Z",
      source: "reconciliation.exposure.v1",
      reason_code: "attribution_ready",
      data_state: "ready",
      recommended_next_action: "Inspect top net contributors first.",
      rows: [],
    }),
  });

  await assert.rejects(
    queryCostAwareAttribution({
      baseUrl: "http://127.0.0.1:8080",
      period: "24h",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "attribution_contract_mismatch");
      assert.match(error.message, /timestamp_utc/i);
      return true;
    },
  );
});
