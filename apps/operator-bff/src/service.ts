import { EventEmitter } from "node:events";

import type {
  OperatorPrincipal,
  ReadinessRunner,
  WorkflowRecord,
  WorkflowStore,
  WorkflowStoreDurability,
} from "./contracts.js";

export class AuthorizationError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "AuthorizationError";
  }
}

function assertCanStart(principal: OperatorPrincipal): void {
  if (principal.role !== "operator" && principal.role !== "admin") {
    throw new AuthorizationError(
      "insufficient_role",
      "Operator or admin role is required to start a workflow.",
    );
  }
}

function assertCanRead(principal: OperatorPrincipal, record: WorkflowRecord): void {
  if (principal.role !== "admin" && principal.sub !== record.operatorId) {
    throw new AuthorizationError(
      "workflow_forbidden",
      "Workflow belongs to a different operator.",
    );
  }
}

function isTerminal(record: WorkflowRecord): boolean {
  return record.status === "succeeded" || record.status === "failed";
}

export class WorkflowService {
  private readonly events = new EventEmitter();

  constructor(
    private readonly store: WorkflowStore,
    private readonly readinessRunner: ReadinessRunner,
  ) {}

  get storageDurability(): WorkflowStoreDurability {
    return this.store.durability;
  }

  async storageReady(): Promise<boolean> {
    return this.store.ready();
  }

  async startReadinessSnapshot(
    principal: OperatorPrincipal,
    idempotencyKey: string,
  ): Promise<WorkflowRecord> {
    assertCanStart(principal);
    const normalizedKey = idempotencyKey.trim();
    if (normalizedKey.length < 8 || normalizedKey.length > 128) {
      throw new AuthorizationError(
        "invalid_idempotency_key",
        "Idempotency-Key must contain between 8 and 128 characters.",
      );
    }

    const { record, created } = await this.store.createOrGet({
      operatorId: principal.sub,
      idempotencyKey: normalizedKey,
      kind: "readiness_snapshot",
    });
    if (created) {
      queueMicrotask(() => {
        void this.executeReadinessSnapshot(record.id);
      });
    }
    return record;
  }

  async getForPrincipal(
    principal: OperatorPrincipal,
    workflowId: string,
  ): Promise<WorkflowRecord | null> {
    const record = await this.store.get(workflowId);
    if (!record) {
      return null;
    }
    assertCanRead(principal, record);
    return record;
  }

  async subscribeForPrincipal(
    principal: OperatorPrincipal,
    workflowId: string,
    listener: (record: WorkflowRecord) => void,
  ): Promise<() => void> {
    const record = await this.getForPrincipal(principal, workflowId);
    if (!record) {
      throw new AuthorizationError("workflow_not_found", "Workflow does not exist.");
    }

    listener(record);
    if (isTerminal(record)) {
      return () => {};
    }

    const eventName = `workflow:${workflowId}`;
    this.events.on(eventName, listener);
    return () => this.events.off(eventName, listener);
  }

  private emit(record: WorkflowRecord): void {
    this.events.emit(`workflow:${record.id}`, record);
  }

  private async executeReadinessSnapshot(workflowId: string): Promise<void> {
    try {
      const running = await this.store.transition(
        workflowId,
        "queued",
        "running",
      );
      this.emit(running);
      const result = await this.readinessRunner();
      const succeeded = await this.store.transition(
        workflowId,
        "running",
        "succeeded",
        { result },
      );
      this.emit(succeeded);
    } catch (error) {
      try {
        const current = await this.store.get(workflowId);
        if (current?.status === "running") {
          const failed = await this.store.transition(
            workflowId,
            "running",
            "failed",
            {
              errorCode:
                error instanceof Error && error.message
                  ? "readiness_probe_failed"
                  : "workflow_failed",
            },
          );
          this.emit(failed);
        }
      } catch {
        // Keep the original workflow failure authoritative; persistence repair is separate.
      }
    }
  }
}
