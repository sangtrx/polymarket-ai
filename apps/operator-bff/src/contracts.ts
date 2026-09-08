export type OperatorRole = "viewer" | "operator" | "admin";

export interface OperatorPrincipal {
  sub: string;
  role: OperatorRole;
}

export type WorkflowKind = "readiness_snapshot";
export type WorkflowStatus = "queued" | "running" | "succeeded" | "failed";

export interface ReadinessSnapshotResult {
  target: string;
  status: number;
  ok: boolean;
  latencyMs: number;
  observedAtUtc: string;
}

export interface WorkflowRecord {
  id: string;
  operatorId: string;
  idempotencyKey: string;
  kind: WorkflowKind;
  status: WorkflowStatus;
  createdAtUtc: string;
  updatedAtUtc: string;
  result?: ReadinessSnapshotResult;
  errorCode?: string;
}

export interface CreateWorkflowInput {
  operatorId: string;
  idempotencyKey: string;
  kind: WorkflowKind;
}

export interface CreateWorkflowResult {
  record: WorkflowRecord;
  created: boolean;
}

export interface WorkflowStore {
  createOrGet(input: CreateWorkflowInput): Promise<CreateWorkflowResult>;
  get(id: string): Promise<WorkflowRecord | null>;
  transition(
    id: string,
    expectedStatus: WorkflowStatus,
    nextStatus: WorkflowStatus,
    patch?: Pick<WorkflowRecord, "result" | "errorCode">,
  ): Promise<WorkflowRecord>;
}

export type ReadinessRunner = () => Promise<ReadinessSnapshotResult>;
