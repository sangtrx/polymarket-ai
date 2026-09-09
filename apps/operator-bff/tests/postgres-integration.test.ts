import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import test from "node:test";

import { Pool } from "pg";

import { PostgresWorkflowStore } from "../src/postgres-store.js";
import { WorkflowService } from "../src/service.js";

const testDatabaseUrl = process.env.OPERATOR_BFF_TEST_DATABASE_URL;

async function waitForTerminal(
  service: WorkflowService,
  principal: { sub: string; role: "operator" },
  workflowId: string,
) {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    const current = await service.getForPrincipal(principal, workflowId);
    if (current?.status === "succeeded" || current?.status === "failed") {
      return current;
    }
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 25));
  }
  assert.fail(`workflow '${workflowId}' did not reach a terminal state`);
}

test(
  "migration and PostgresWorkflowStore preserve durable idempotent workflow semantics",
  { skip: !testDatabaseUrl },
  async () => {
    assert.ok(testDatabaseUrl);
    const parsed = new URL(testDatabaseUrl);
    assert.equal(
      parsed.pathname,
      "/operator_bff_test",
      "integration smoke may only mutate the dedicated operator_bff_test database",
    );

    const pool = new Pool({ connectionString: testDatabaseUrl });
    try {
      await pool.query("DROP TABLE IF EXISTS operator_workflow_requests");
      const migrationSql = await readFile(
        resolve(process.cwd(), "migrations/001_operator_workflow_requests.sql"),
        "utf8",
      );
      await pool.query(migrationSql);

      const store = new PostgresWorkflowStore(pool);
      assert.equal(await store.ready(), true);

      const service = new WorkflowService(store, async () => ({
        target: "http://control-api.test/health",
        status: 200,
        ok: true,
        latencyMs: 7,
        observedAtUtc: new Date().toISOString(),
      }));
      const principal = { sub: "ci-operator", role: "operator" } as const;
      const idempotencyKey = "postgres-ci-idempotency-001";

      const first = await service.startReadinessSnapshot(principal, idempotencyKey);
      const terminal = await waitForTerminal(service, principal, first.id);
      assert.equal(terminal.status, "succeeded");
      assert.equal(terminal.result?.ok, true);
      assert.equal(terminal.result?.latencyMs, 7);

      const repeated = await service.startReadinessSnapshot(
        principal,
        idempotencyKey,
      );
      assert.equal(repeated.id, first.id);
      assert.equal(repeated.status, "succeeded");

      const count = await pool.query<{ count: string }>(
        "SELECT COUNT(*)::text AS count FROM operator_workflow_requests WHERE operator_id = $1 AND workflow_kind = 'readiness_snapshot' AND idempotency_key = $2",
        [principal.sub, idempotencyKey],
      );
      assert.equal(count.rows[0]?.count, "1");
    } finally {
      await pool.end();
    }
  },
);
