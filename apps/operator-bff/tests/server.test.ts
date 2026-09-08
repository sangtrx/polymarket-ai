import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import test from "node:test";

import type { ReadinessSnapshotResult } from "../src/contracts.js";
import { WorkflowService } from "../src/service.js";
import { buildServer } from "../src/server.js";
import { InMemoryWorkflowStore } from "../src/store.js";

const SECRET = "0123456789abcdef0123456789abcdef";
const RESULT: ReadinessSnapshotResult = {
  target: "http://127.0.0.1:8080/health",
  status: 200,
  ok: true,
  latencyMs: 4,
  observedAtUtc: "2026-09-09T00:00:00.000Z",
};

function signToken(payload: Record<string, unknown>): string {
  const header = Buffer.from(JSON.stringify({ alg: "HS256", typ: "JWT" })).toString(
    "base64url",
  );
  const body = Buffer.from(JSON.stringify(payload)).toString("base64url");
  const signature = createHmac("sha256", SECRET)
    .update(`${header}.${body}`)
    .digest("base64url");
  return `${header}.${body}.${signature}`;
}

async function withServer(
  callback: (baseUrl: string) => Promise<void>,
): Promise<void> {
  const workflowService = new WorkflowService(
    new InMemoryWorkflowStore(),
    async () => RESULT,
  );
  const server = buildServer({ authSecret: SECRET, workflowService });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  assert.ok(address && typeof address === "object");
  try {
    await callback(`http://127.0.0.1:${address.port}`);
  } finally {
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
}

test("health and readiness endpoints distinguish process health from auth readiness", async () => {
  const server = buildServer({ authSecret: "too-short" });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  assert.ok(address && typeof address === "object");
  const baseUrl = `http://127.0.0.1:${address.port}`;
  try {
    const health = await fetch(`${baseUrl}/healthz`);
    const readiness = await fetch(`${baseUrl}/readyz`);
    assert.equal(health.status, 200);
    assert.equal(readiness.status, 503);
  } finally {
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});

test("workflow HTTP route enforces auth and idempotency", async () => {
  await withServer(async (baseUrl) => {
    const unauthorized = await fetch(`${baseUrl}/v1/workflows/readiness-snapshots`, {
      method: "POST",
      headers: { "idempotency-key": "http-test-0001" },
    });
    assert.equal(unauthorized.status, 401);

    const token = signToken({
      sub: "operator-http",
      role: "operator",
      exp: Math.floor(Date.now() / 1000) + 60,
    });
    const headers = {
      authorization: `Bearer ${token}`,
      "idempotency-key": "http-test-0001",
    };
    const first = await fetch(`${baseUrl}/v1/workflows/readiness-snapshots`, {
      method: "POST",
      headers,
    });
    const second = await fetch(`${baseUrl}/v1/workflows/readiness-snapshots`, {
      method: "POST",
      headers,
    });
    assert.equal(first.status, 202);
    assert.equal(second.status, 202);

    const firstBody = (await first.json()) as { workflow_id: string };
    const secondBody = (await second.json()) as { workflow_id: string };
    assert.equal(secondBody.workflow_id, firstBody.workflow_id);

    let terminalStatus = "";
    for (let attempt = 0; attempt < 50; attempt += 1) {
      const statusResponse = await fetch(
        `${baseUrl}/v1/workflows/${firstBody.workflow_id}`,
        { headers: { authorization: `Bearer ${token}` } },
      );
      assert.equal(statusResponse.status, 200);
      const body = (await statusResponse.json()) as { status: string };
      terminalStatus = body.status;
      if (terminalStatus === "succeeded") {
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 1));
    }
    assert.equal(terminalStatus, "succeeded");
  });
});
