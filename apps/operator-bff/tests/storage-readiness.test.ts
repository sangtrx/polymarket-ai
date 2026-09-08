import assert from "node:assert/strict";
import test from "node:test";

import type {
  CreateWorkflowInput,
  CreateWorkflowResult,
  WorkflowRecord,
  WorkflowStatus,
  WorkflowStore,
} from "../src/contracts.js";
import { WorkflowService } from "../src/service.js";
import { buildServer } from "../src/server.js";
import { InMemoryWorkflowStore } from "../src/store.js";

const SECRET = "0123456789abcdef0123456789abcdef";

async function readinessStatus(workflowService: WorkflowService): Promise<{
  status: number;
  body: Record<string, unknown>;
}> {
  const server = buildServer({ authSecret: SECRET, workflowService });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  assert.ok(address && typeof address === "object");
  try {
    const response = await fetch(`http://127.0.0.1:${address.port}/readyz`);
    return {
      status: response.status,
      body: (await response.json()) as Record<string, unknown>,
    };
  } finally {
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
}

class ReadyOnlyPostgresStore implements WorkflowStore {
  readonly durability = "postgres" as const;

  constructor(private readonly available: boolean) {}

  async ready(): Promise<boolean> {
    return this.available;
  }

  async createOrGet(_input: CreateWorkflowInput): Promise<CreateWorkflowResult> {
    throw new Error("not used by readiness test");
  }

  async get(_id: string): Promise<WorkflowRecord | null> {
    throw new Error("not used by readiness test");
  }

  async transition(
    _id: string,
    _expectedStatus: WorkflowStatus,
    _nextStatus: WorkflowStatus,
  ): Promise<WorkflowRecord> {
    throw new Error("not used by readiness test");
  }
}

const runner = async () => ({
  target: "http://127.0.0.1:8080/health",
  status: 200,
  ok: true,
  latencyMs: 1,
  observedAtUtc: "2026-09-09T00:00:00.000Z",
});

test("readiness rejects in-memory storage even when auth is configured", async () => {
  const result = await readinessStatus(
    new WorkflowService(new InMemoryWorkflowStore(), runner),
  );
  assert.equal(result.status, 503);
  assert.equal(result.body.storage_durability, "memory");
  assert.equal(result.body.durable_storage_ready, false);
});

test("readiness requires reachable durable storage", async () => {
  const unavailable = await readinessStatus(
    new WorkflowService(new ReadyOnlyPostgresStore(false), runner),
  );
  assert.equal(unavailable.status, 503);
  assert.equal(unavailable.body.storage_durability, "postgres");
  assert.equal(unavailable.body.durable_storage_ready, false);

  const available = await readinessStatus(
    new WorkflowService(new ReadyOnlyPostgresStore(true), runner),
  );
  assert.equal(available.status, 200);
  assert.equal(available.body.status, "ready");
  assert.equal(available.body.durable_storage_ready, true);
});
