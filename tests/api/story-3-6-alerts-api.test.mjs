import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { rmSync, mkdtempSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const PROJECT_ROOT = resolve(".");
const OPERATOR_CONSOLE_FILTER = "operator-console";
const ALERT_SOURCE_PATH = "src/lib/incidents/alerts.ts";

let compiledOutputDir;
let alertsModule;

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
  if (alertsModule) {
    return alertsModule;
  }

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-3-6-alerts-api-"));
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
      ALERT_SOURCE_PATH,
    ],
    { cwd: PROJECT_ROOT, encoding: "utf8" },
  );
  assert.equal(compileResult.status, 0, compileResult.stderr || compileResult.stdout);

  const compiledPath = findFileRecursively(compiledOutputDir, "alerts.js");
  assert.ok(compiledPath, "expected compiled alerts.js output");

  const require = createRequire(import.meta.url);
  alertsModule = require(compiledPath);
  return alertsModule;
}

test.after(() => {
  if (compiledOutputDir) {
    rmSync(compiledOutputDir, { recursive: true, force: true });
  }
});

test("Story 3.6 alert API maps canonical endpoint and guidance-rich alert payload", async () => {
  const { queryIncidentAlerts } = loadCompiledModule();

  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    return {
      ok: true,
      status: 200,
      json: async () => ({
        status: "accepted",
        action: "incident_alerts_query",
        actor_id: "ops-1",
        role: "operational_control",
        correlation_id: "corr-alert-api-001",
        timestamp_utc: "2026-04-06T16:30:00.000Z",
        source: "control-api.incident-alerts.v1",
        reason_code: "alert_ready",
        data_state: "ready",
        recommended_next_action:
          "Review alert cause and execute linked containment runbook if severity is critical.",
        alerts: [
          {
            alert_id: "incident::alert::001",
            severity: "critical",
            impacted_subsystem: "reconciliation",
            cause: "Reconciliation lag exceeded 60 seconds for active portfolio scope.",
            recommended_next_action:
              "Trigger reduce-only mode and verify reconciler backlog health.",
            evidence_link:
              "https://docs.example.com/operations/severity-alert-delivery#reconciliation-lag",
            issued_at: "2026-04-06T16:29:45.000Z",
            correlation_id: "corr-alert-api-001",
            reason_code: "alert_reconciliation_lag_exceeded",
            status: "delivered",
            delivered_at: "2026-04-06T16:29:50.000Z",
            attempts: [
              {
                attempt_number: 1,
                channel: "pagerduty",
                outcome: "failed",
                reason_code: "alert_delivery_primary_failed",
                attempted_at: "2026-04-06T16:29:47.000Z",
                failed_at: "2026-04-06T16:29:47.000Z",
              },
              {
                attempt_number: 2,
                channel: "slack",
                outcome: "delivered",
                reason_code: "alert_ready",
                attempted_at: "2026-04-06T16:29:50.000Z",
                delivered_at: "2026-04-06T16:29:50.000Z",
              },
            ],
          },
        ],
      }),
    };
  };

  const result = await queryIncidentAlerts({
    baseUrl: "http://127.0.0.1:8080",
    limit: 25,
    fetchImpl,
  });

  assert.equal(requests.length, 1);
  assert.equal(
    requests[0].url,
    "http://127.0.0.1:8080/control/incidents/alerts?limit=25",
  );
  assert.equal(requests[0].init.method, "GET");
  assert.equal(result.status, "accepted");
  assert.equal(result.dataState, "ready");
  assert.equal(result.alerts[0].severity, "critical");
  assert.equal(
    result.alerts[0].recommendedNextAction,
    "Trigger reduce-only mode and verify reconciler backlog health.",
  );
  assert.equal(
    result.alerts[0].evidenceLink,
    "https://docs.example.com/operations/severity-alert-delivery#reconciliation-lag",
  );
  parseIsoTimestamp(result.alerts[0].issuedAt);
  parseIsoTimestamp(result.alerts[0].deliveredAt);
  const dispatchLatencyMs =
    Date.parse(result.alerts[0].deliveredAt) - Date.parse(result.alerts[0].issuedAt);
  assert.ok(
    dispatchLatencyMs <= 30_000,
    `expected critical dispatch latency <= 30s, received ${dispatchLatencyMs}ms`,
  );
  assert.equal(result.alerts[0].attempts.length, 2);
  assert.equal(result.alerts[0].attempts[0].attemptNumber, 1);
  assert.equal(result.alerts[0].attempts[0].channel, "pagerduty");
  assert.equal(result.alerts[0].attempts[0].outcome, "failed");
  assert.equal(result.alerts[0].attempts[1].attemptNumber, 2);
  assert.equal(result.alerts[0].attempts[1].channel, "slack");
  assert.equal(result.alerts[0].attempts[1].outcome, "delivered");
});

test("Story 3.6 alert API surfaces dependency-unavailable machine errors", async () => {
  const { queryIncidentAlerts } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: false,
    status: 503,
    json: async () => ({
      error_code: "alert_dependency_unavailable",
      reason_code: "alert_dependency_unavailable",
      message: "incident alert dependencies are unavailable",
      action: "incident_alerts_query",
      correlation_id: "corr-alert-503",
      timestamp_utc: "2026-04-06T16:30:01.000Z",
      endpoint: "/control/incidents/alerts?limit=25",
      source: "control-api.incident-alerts.v1",
      occurred_at: "2026-04-06T16:30:01.000Z",
    }),
  });

  await assert.rejects(
    queryIncidentAlerts({
      baseUrl: "http://127.0.0.1:8080",
      limit: 25,
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 503);
      assert.equal(error.errorCode, "alert_dependency_unavailable");
      assert.equal(error.reasonCode, "alert_dependency_unavailable");
      assert.equal(error.action, "incident_alerts_query");
      assert.equal(error.correlationId, "corr-alert-503");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );
});

test("Story 3.6 alert API surfaces unauthorized machine errors for restricted alert queries", async () => {
  const { queryIncidentAlerts } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: false,
    status: 401,
    json: async () => ({
      error_code: "alert_unauthorized",
      reason_code: "alert_unauthorized",
      message: "operator role is required for incident alerts query",
      action: "incident_alerts_query",
      correlation_id: "corr-alert-401",
      timestamp_utc: "2026-04-06T16:30:02.000Z",
      endpoint: "/control/incidents/alerts?limit=25",
      source: "control-api.incident-alerts.v1",
      occurred_at: "2026-04-06T16:30:02.000Z",
    }),
  });

  await assert.rejects(
    queryIncidentAlerts({
      baseUrl: "http://127.0.0.1:8080",
      limit: 25,
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 401);
      assert.equal(error.errorCode, "alert_unauthorized");
      assert.equal(error.reasonCode, "alert_unauthorized");
      assert.equal(error.action, "incident_alerts_query");
      assert.equal(error.correlationId, "corr-alert-401");
      parseIsoTimestamp(error.timestampUtc);
      return true;
    },
  );
});

test("Story 3.6 alert API rejects malformed success payloads with explicit contract mismatch", async () => {
  const { queryIncidentAlerts } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      status: "accepted",
      action: "incident_alerts_query",
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-alert-contract",
      timestamp_utc: "2026-04-06T16:30:00.000Z",
      source: "control-api.incident-alerts.v1",
      reason_code: "alert_ready",
      data_state: "ready",
      recommended_next_action: "Inspect alert evidence.",
      alerts: [
        {
          alert_id: "incident::alert::001",
          severity: "critical",
          impacted_subsystem: "reconciliation",
          cause: "reconciliation lag breached threshold",
          evidence_link:
            "https://docs.example.com/operations/severity-alert-delivery#reconciliation-lag",
          issued_at: "2026-04-06T16:29:45.000Z",
          correlation_id: "corr-alert-contract",
          reason_code: "alert_reconciliation_lag_exceeded",
          status: "delivered",
          delivered_at: "2026-04-06T16:29:47.000Z",
          attempts: [],
        },
      ],
    }),
  });

  await assert.rejects(
    queryIncidentAlerts({
      baseUrl: "http://127.0.0.1:8080",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "incident_alert_contract_mismatch");
      assert.match(error.message, /recommended_next_action/i);
      return true;
    },
  );
});

test("Story 3.6 alert API rejects alert payloads with malformed evidence links", async () => {
  const { queryIncidentAlerts } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      status: "accepted",
      action: "incident_alerts_query",
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-alert-evidence",
      timestamp_utc: "2026-04-06T16:30:00.000Z",
      source: "control-api.incident-alerts.v1",
      reason_code: "alert_ready",
      data_state: "ready",
      recommended_next_action: "Inspect alert evidence.",
      alerts: [
        {
          alert_id: "incident::alert::001",
          severity: "critical",
          impacted_subsystem: "reconciliation",
          cause: "reconciliation lag breached threshold",
          recommended_next_action: "execute containment",
          evidence_link: "/operations/severity-alert-delivery#reconciliation-lag",
          issued_at: "2026-04-06T16:29:45.000Z",
          correlation_id: "corr-alert-evidence",
          reason_code: "alert_reconciliation_lag_exceeded",
          status: "delivered",
          delivered_at: "2026-04-06T16:29:47.000Z",
          attempts: [],
        },
      ],
    }),
  });

  await assert.rejects(
    queryIncidentAlerts({
      baseUrl: "http://127.0.0.1:8080",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "incident_alert_contract_mismatch");
      assert.match(error.message, /evidence_link/i);
      return true;
    },
  );
});

test("Story 3.6 alert API rejects non-integer limit values before issuing the request", async () => {
  const { queryIncidentAlerts } = loadCompiledModule();

  await assert.rejects(
    queryIncidentAlerts({
      baseUrl: "http://127.0.0.1:8080",
      limit: 10.5,
      fetchImpl: async () => {
        throw new Error("fetch should not be called for invalid limit");
      },
    }),
    (error) => {
      assert.equal(error.status, 400);
      assert.equal(error.errorCode, "alert_invalid_payload");
      assert.match(error.message, /integer between 1 and 200/i);
      return true;
    },
  );
});

test("Story 3.6 alert API rejects delivered alert payloads missing delivered_at", async () => {
  const { queryIncidentAlerts } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      status: "accepted",
      action: "incident_alerts_query",
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-alert-contract",
      timestamp_utc: "2026-04-06T16:30:00.000Z",
      source: "control-api.incident-alerts.v1",
      reason_code: "alert_ready",
      data_state: "ready",
      recommended_next_action: "Inspect alert evidence.",
      alerts: [
        {
          alert_id: "incident::alert::001",
          severity: "critical",
          impacted_subsystem: "reconciliation",
          cause: "reconciliation lag breached threshold",
          recommended_next_action: "execute containment",
          evidence_link:
            "https://docs.example.com/operations/severity-alert-delivery#reconciliation-lag",
          issued_at: "2026-04-06T16:29:45.000Z",
          correlation_id: "corr-alert-contract",
          reason_code: "alert_reconciliation_lag_exceeded",
          status: "delivered",
          attempts: [],
        },
      ],
    }),
  });

  await assert.rejects(
    queryIncidentAlerts({
      baseUrl: "http://127.0.0.1:8080",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "incident_alert_contract_mismatch");
      assert.match(error.message, /delivered_at/i);
      return true;
    },
  );
});

test("Story 3.6 alert API rejects delivered attempts missing delivered_at", async () => {
  const { queryIncidentAlerts } = loadCompiledModule();

  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({
      status: "accepted",
      action: "incident_alerts_query",
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-alert-contract",
      timestamp_utc: "2026-04-06T16:30:00.000Z",
      source: "control-api.incident-alerts.v1",
      reason_code: "alert_ready",
      data_state: "ready",
      recommended_next_action: "Inspect alert evidence.",
      alerts: [
        {
          alert_id: "incident::alert::001",
          severity: "critical",
          impacted_subsystem: "reconciliation",
          cause: "reconciliation lag breached threshold",
          recommended_next_action: "execute containment",
          evidence_link:
            "https://docs.example.com/operations/severity-alert-delivery#reconciliation-lag",
          issued_at: "2026-04-06T16:29:45.000Z",
          correlation_id: "corr-alert-contract",
          reason_code: "alert_reconciliation_lag_exceeded",
          status: "delivered",
          delivered_at: "2026-04-06T16:29:47.000Z",
          attempts: [
            {
              attempt_number: 1,
              channel: "pagerduty",
              outcome: "delivered",
              reason_code: "alert_ready",
              attempted_at: "2026-04-06T16:29:47.000Z",
            },
          ],
        },
      ],
    }),
  });

  await assert.rejects(
    queryIncidentAlerts({
      baseUrl: "http://127.0.0.1:8080",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 502);
      assert.equal(error.errorCode, "incident_alert_contract_mismatch");
      assert.match(error.message, /delivered_at/i);
      return true;
    },
  );
});
