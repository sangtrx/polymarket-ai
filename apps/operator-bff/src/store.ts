import { randomUUID } from "node:crypto";

import type {
  CreateWorkflowInput,
  CreateWorkflowResult,
  WorkflowRecord,
  WorkflowStatus,
  WorkflowStore,
} from "./contracts.js";

function cloneRecord(record: WorkflowRecord): WorkflowRecord {
  return structuredClone(record);
}

export class InMemoryWorkflowStore implements WorkflowStore {
  private readonly records = new Map<string, WorkflowRecord>();
  private readonly idempotencyIndex = new Map<string, string>();

  async createOrGet(input: CreateWorkflowInput): Promise<CreateWorkflowResult> {
    const indexKey = `${input.operatorId}:${input.kind}:${input.idempotencyKey}`;
    const existingId = this.idempotencyIndex.get(indexKey);
    if (existingId) {
      const existing = this.records.get(existingId);
      if (!existing) {
        throw new Error("idempotency index points to a missing workflow");
      }
      return { record: cloneRecord(existing), created: false };
    }

    const now = new Date().toISOString();
    const record: WorkflowRecord = {
      id: randomUUID(),
      operatorId: input.operatorId,
      idempotencyKey: input.idempotencyKey,
      kind: input.kind,
      status: "queued",
      createdAtUtc: now,
      updatedAtUtc: now,
    };
    this.records.set(record.id, record);
    this.idempotencyIndex.set(indexKey, record.id);
    return { record: cloneRecord(record), created: true };
  }

  async get(id: string): Promise<WorkflowRecord | null> {
    const record = this.records.get(id);
    return record ? cloneRecord(record) : null;
  }

  async transition(
    id: string,
    expectedStatus: WorkflowStatus,
    nextStatus: WorkflowStatus,
    patch: Pick<WorkflowRecord, "result" | "errorCode"> = {},
  ): Promise<WorkflowRecord> {
    const current = this.records.get(id);
    if (!current) {
      throw new Error(`workflow '${id}' does not exist`);
    }
    if (current.status !== expectedStatus) {
      throw new Error(
        `workflow '${id}' expected '${expectedStatus}' but is '${current.status}'`,
      );
    }

    const next: WorkflowRecord = {
      ...current,
      ...patch,
      status: nextStatus,
      updatedAtUtc: new Date().toISOString(),
    };
    this.records.set(id, next);
    return cloneRecord(next);
  }
}
