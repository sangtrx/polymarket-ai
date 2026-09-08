import { createServer, type IncomingMessage, type ServerResponse } from "node:http";

import { AuthenticationError, verifyOperatorBearerToken } from "./auth.js";
import type { OperatorPrincipal, ReadinessSnapshotResult, WorkflowRecord } from "./contracts.js";
import { PostgresWorkflowStore } from "./postgres-store.js";
import { AuthorizationError, WorkflowService } from "./service.js";
import { InMemoryWorkflowStore } from "./store.js";

const DEFAULT_CONTROL_API_BASE_URL = "http://127.0.0.1:8080";

interface ErrorEnvelope {
  error_code: string;
  message: string;
  timestamp_utc: string;
}

function jsonResponse(
  response: ServerResponse,
  status: number,
  payload: unknown,
): void {
  response.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  response.end(JSON.stringify(payload));
}

function errorEnvelope(errorCode: string, message: string): ErrorEnvelope {
  return {
    error_code: errorCode,
    message,
    timestamp_utc: new Date().toISOString(),
  };
}

function workflowEnvelope(record: WorkflowRecord): Record<string, unknown> {
  return {
    workflow_id: record.id,
    operator_id: record.operatorId,
    idempotency_key: record.idempotencyKey,
    kind: record.kind,
    status: record.status,
    created_at_utc: record.createdAtUtc,
    updated_at_utc: record.updatedAtUtc,
    result: record.result ?? null,
    error_code: record.errorCode ?? null,
  };
}

function authenticate(request: IncomingMessage, secret: string): OperatorPrincipal {
  const authorization = Array.isArray(request.headers.authorization)
    ? request.headers.authorization[0]
    : request.headers.authorization;
  return verifyOperatorBearerToken(authorization, secret);
}

function parseWorkflowPath(pathname: string): { id: string; events: boolean } | null {
  const match = pathname.match(/^\/v1\/workflows\/([0-9a-f-]+)(\/events)?$/i);
  if (!match?.[1]) {
    return null;
  }
  return { id: match[1], events: match[2] === "/events" };
}

export function createReadinessRunner(
  controlApiBaseUrl: string,
  timeoutMs = 5000,
): () => Promise<ReadinessSnapshotResult> {
  const target = `${controlApiBaseUrl.replace(/\/$/, "")}/health`;
  return async () => {
    const startedAt = performance.now();
    const response = await fetch(target, {
      method: "GET",
      headers: { accept: "application/json" },
      signal: AbortSignal.timeout(timeoutMs),
    });
    return {
      target,
      status: response.status,
      ok: response.ok,
      latencyMs: Math.max(0, Math.round(performance.now() - startedAt)),
      observedAtUtc: new Date().toISOString(),
    };
  };
}

export function buildServer(options?: {
  authSecret?: string;
  controlApiBaseUrl?: string;
  databaseUrl?: string;
  workflowService?: WorkflowService;
}) {
  const authSecret = options?.authSecret ?? process.env.OPERATOR_BFF_AUTH_SECRET ?? "";
  const databaseUrl = options?.databaseUrl ?? process.env.OPERATOR_BFF_DATABASE_URL ?? "";
  const workflowService =
    options?.workflowService ??
    new WorkflowService(
      databaseUrl
        ? PostgresWorkflowStore.fromConnectionString(databaseUrl)
        : new InMemoryWorkflowStore(),
      createReadinessRunner(
        options?.controlApiBaseUrl ??
          process.env.CONTROL_API_BASE_URL ??
          DEFAULT_CONTROL_API_BASE_URL,
      ),
    );

  return createServer(async (request, response) => {
    const url = new URL(request.url ?? "/", "http://operator-bff.local");

    if (request.method === "GET" && url.pathname === "/healthz") {
      jsonResponse(response, 200, { status: "ok", service: "operator-bff" });
      return;
    }

    if (request.method === "GET" && url.pathname === "/readyz") {
      const authConfigured = authSecret.length >= 32;
      const durableStorageConfigured = workflowService.storageDurability === "postgres";
      const durableStorageReady =
        durableStorageConfigured && (await workflowService.storageReady());
      const ready = authConfigured && durableStorageReady;
      jsonResponse(response, ready ? 200 : 503, {
        status: ready ? "ready" : "not_ready",
        auth_configured: authConfigured,
        storage_durability: workflowService.storageDurability,
        durable_storage_ready: durableStorageReady,
      });
      return;
    }

    try {
      const principal = authenticate(request, authSecret);

      if (
        request.method === "POST" &&
        url.pathname === "/v1/workflows/readiness-snapshots"
      ) {
        const rawIdempotency = request.headers["idempotency-key"];
        const idempotencyKey = Array.isArray(rawIdempotency)
          ? rawIdempotency[0]
          : rawIdempotency;
        if (!idempotencyKey) {
          jsonResponse(
            response,
            400,
            errorEnvelope("missing_idempotency_key", "Idempotency-Key header is required."),
          );
          return;
        }
        const record = await workflowService.startReadinessSnapshot(
          principal,
          idempotencyKey,
        );
        jsonResponse(response, 202, workflowEnvelope(record));
        return;
      }

      const workflowPath = parseWorkflowPath(url.pathname);
      if (request.method === "GET" && workflowPath && !workflowPath.events) {
        const record = await workflowService.getForPrincipal(principal, workflowPath.id);
        if (!record) {
          jsonResponse(
            response,
            404,
            errorEnvelope("workflow_not_found", "Workflow does not exist."),
          );
          return;
        }
        jsonResponse(response, 200, workflowEnvelope(record));
        return;
      }

      if (request.method === "GET" && workflowPath?.events) {
        response.writeHead(200, {
          "content-type": "text/event-stream; charset=utf-8",
          "cache-control": "no-cache, no-transform",
          connection: "keep-alive",
        });
        let unsubscribe = () => {};
        unsubscribe = await workflowService.subscribeForPrincipal(
          principal,
          workflowPath.id,
          (record) => {
            response.write(`event: workflow\ndata: ${JSON.stringify(workflowEnvelope(record))}\n\n`);
            if (record.status === "succeeded" || record.status === "failed") {
              unsubscribe();
              response.end();
            }
          },
        );
        request.on("close", unsubscribe);
        return;
      }

      jsonResponse(response, 404, errorEnvelope("route_not_found", "Route not found."));
    } catch (error) {
      if (error instanceof AuthenticationError) {
        jsonResponse(response, 401, errorEnvelope(error.code, error.message));
        return;
      }
      if (error instanceof AuthorizationError) {
        const status = error.code === "workflow_not_found" ? 404 : 403;
        jsonResponse(response, status, errorEnvelope(error.code, error.message));
        return;
      }
      jsonResponse(
        response,
        500,
        errorEnvelope("operator_bff_internal_error", "Operator BFF request failed."),
      );
    }
  });
}
