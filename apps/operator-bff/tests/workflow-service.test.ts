import assert from "node:assert/strict";
import test from "node:test";

import type { ReadinessSnapshotResult, WorkflowStatus } from "../src/contracts.js";
import { WorkflowService } from "../src/service.js";
import { InMemoryWorkflowStore } from "../src/store.js";

const PRINCIPAL = { sub: "operator-7", role: "operator" as const };
const RESULT: ReadinessSnapshotResult = {
  target: "http://127.0.0.1:8080/health",
  status: 200,
  ok: true,
  latencyMs: 9,
  observedAtUtc: "2026-09-09T00:00:00.000Z",
};

async function waitForStatus(
  service: WorkflowService,
  id: string,
  status: WorkflowStatus,
): Promise<void> {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const current = await service.getForPrincipal(PRINCIPAL, id);
    if (current?.status === status) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
  assert.fail(`workflow '${id}' did not reach '${status}'`);
}

test("readiness workflow is idempotent per operator and key", async () => {
  const service = new WorkflowService(new InMemoryWorkflowStore(), async () => RESULT);
  const first = await service.startReadinessSnapshot(PRINCIPAL, "snapshot-0001");
  const second = await service.startReadinessSnapshot(PRINCIPAL, "snapshot-0001");

  assert.equal(second.id, first.id);
  await waitForStatus(service, first.id, "succeeded");
  const terminal = await service.getForPrincipal(PRINCIPAL, first.id);
  assert.equal(terminal?.status, "succeeded");
  assert.deepEqual(terminal?.result, RESULT);
});

test("readiness workflow publishes terminal state to subscribers", async () => {
  let release: ((value: ReadinessSnapshotResult) => void) | undefined;
  const service = new WorkflowService(
    new InMemoryWorkflowStore(),
    () =>
      new Promise<ReadinessSnapshotResult>((resolve) => {
        release = resolve;
      }),
  );
  const workflow = await service.startReadinessSnapshot(PRINCIPAL, "snapshot-0002");
  await waitForStatus(service, workflow.id, "running");

  const observed: WorkflowStatus[] = [];
  const unsubscribe = await service.subscribeForPrincipal(
    PRINCIPAL,
    workflow.id,
    (record) => observed.push(record.status),
  );
  assert.ok(release);
  release(RESULT);
  await waitForStatus(service, workflow.id, "succeeded");
  unsubscribe();

  assert.deepEqual(observed, ["running", "succeeded"]);
});

test("runner failure becomes explicit failed workflow state", async () => {
  const service = new WorkflowService(new InMemoryWorkflowStore(), async () => {
    throw new Error("upstream unavailable");
  });
  const workflow = await service.startReadinessSnapshot(PRINCIPAL, "snapshot-0003");
  await waitForStatus(service, workflow.id, "failed");
  const terminal = await service.getForPrincipal(PRINCIPAL, workflow.id);

  assert.equal(terminal?.status, "failed");
  assert.equal(terminal?.errorCode, "readiness_probe_failed");
});

test("viewer cannot start workflow and operators cannot read another operator workflow", async () => {
  const service = new WorkflowService(new InMemoryWorkflowStore(), async () => RESULT);
  await assert.rejects(
    () =>
      service.startReadinessSnapshot(
        { sub: "viewer-1", role: "viewer" },
        "snapshot-0004",
      ),
    /Operator or admin role is required/,
  );

  const workflow = await service.startReadinessSnapshot(PRINCIPAL, "snapshot-0005");
  await assert.rejects(
    () =>
      service.getForPrincipal(
        { sub: "operator-8", role: "operator" },
        workflow.id,
      ),
    /different operator/,
  );
});
