import { randomUUID } from "node:crypto";

import { Pool, type PoolConfig, type QueryResultRow } from "pg";

import type {
  CreateWorkflowInput,
  CreateWorkflowResult,
  ReadinessSnapshotResult,
  WorkflowKind,
  WorkflowRecord,
  WorkflowStatus,
  WorkflowStore,
} from "./contracts.js";

interface WorkflowRow extends QueryResultRow {
  workflow_id: string;
  operator_id: string;
  idempotency_key: string;
  workflow_kind: WorkflowKind;
  workflow_status: WorkflowStatus;
  result: ReadinessSnapshotResult | null;
  error_code: string | null;
  created_at: Date | string;
  updated_at: Date | string;
}

function iso(value: Date | string): string {
  return value instanceof Date ? value.toISOString() : new Date(value).toISOString();
}

function toRecord(row: WorkflowRow): WorkflowRecord {
  return {
    id: row.workflow_id,
    operatorId: row.operator_id,
    idempotencyKey: row.idempotency_key,
    kind: row.workflow_kind,
    status: row.workflow_status,
    createdAtUtc: iso(row.created_at),
    updatedAtUtc: iso(row.updated_at),
    ...(row.result ? { result: row.result } : {}),
    ...(row.error_code ? { errorCode: row.error_code } : {}),
  };
}

const RETURNING_COLUMNS = `
  workflow_id,
  operator_id,
  idempotency_key,
  workflow_kind,
  workflow_status,
  result,
  error_code,
  created_at,
  updated_at
`;

export class PostgresWorkflowStore implements WorkflowStore {
  readonly durability = "postgres" as const;

  constructor(private readonly pool: Pool) {}

  static fromConnectionString(connectionString: string): PostgresWorkflowStore {
    const config: PoolConfig = { connectionString };
    return new PostgresWorkflowStore(new Pool(config));
  }

  async ready(): Promise<boolean> {
    try {
      const result = await this.pool.query<{ table_name: string | null }>(
        "SELECT to_regclass('public.operator_workflow_requests')::text AS table_name",
      );
      return result.rows[0]?.table_name === "operator_workflow_requests";
    } catch {
      return false;
    }
  }

  async createOrGet(input: CreateWorkflowInput): Promise<CreateWorkflowResult> {
    const workflowId = randomUUID();
    const inserted = await this.pool.query<WorkflowRow>(
      `INSERT INTO operator_workflow_requests (
         workflow_id, operator_id, idempotency_key, workflow_kind, workflow_status
       ) VALUES ($1, $2, $3, $4, 'queued')
       ON CONFLICT (operator_id, workflow_kind, idempotency_key) DO NOTHING
       RETURNING ${RETURNING_COLUMNS}`,
      [workflowId, input.operatorId, input.idempotencyKey, input.kind],
    );
    if (inserted.rows[0]) {
      return { record: toRecord(inserted.rows[0]), created: true };
    }

    const existing = await this.pool.query<WorkflowRow>(
      `SELECT ${RETURNING_COLUMNS}
       FROM operator_workflow_requests
       WHERE operator_id = $1 AND workflow_kind = $2 AND idempotency_key = $3`,
      [input.operatorId, input.kind, input.idempotencyKey],
    );
    if (!existing.rows[0]) {
      throw new Error("workflow idempotency conflict resolved without an existing row");
    }
    return { record: toRecord(existing.rows[0]), created: false };
  }

  async get(id: string): Promise<WorkflowRecord | null> {
    const result = await this.pool.query<WorkflowRow>(
      `SELECT ${RETURNING_COLUMNS}
       FROM operator_workflow_requests
       WHERE workflow_id = $1`,
      [id],
    );
    return result.rows[0] ? toRecord(result.rows[0]) : null;
  }

  async transition(
    id: string,
    expectedStatus: WorkflowStatus,
    nextStatus: WorkflowStatus,
    patch: Pick<WorkflowRecord, "result" | "errorCode"> = {},
  ): Promise<WorkflowRecord> {
    const result = await this.pool.query<WorkflowRow>(
      `UPDATE operator_workflow_requests
       SET workflow_status = $3,
           result = $4::jsonb,
           error_code = $5,
           updated_at = NOW()
       WHERE workflow_id = $1 AND workflow_status = $2
       RETURNING ${RETURNING_COLUMNS}`,
      [
        id,
        expectedStatus,
        nextStatus,
        patch.result ? JSON.stringify(patch.result) : null,
        patch.errorCode ?? null,
      ],
    );
    if (result.rows[0]) {
      return toRecord(result.rows[0]);
    }

    const current = await this.get(id);
    if (!current) {
      throw new Error(`workflow '${id}' does not exist`);
    }
    throw new Error(
      `workflow '${id}' expected '${expectedStatus}' but is '${current.status}'`,
    );
  }

  async close(): Promise<void> {
    await this.pool.end();
  }
}
