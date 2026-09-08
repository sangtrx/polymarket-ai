import { createHmac, randomUUID } from "node:crypto";

const LOCAL_OPERATOR_BFF_URL = "http://127.0.0.1:8090";
const TOKEN_TTL_SECONDS = 60;

type OperatorRole = "operator" | "admin";

interface OperatorBffServerConfig {
  baseUrl: string;
  authSecret: string;
  operatorId: string;
  role: OperatorRole;
}

export class OperatorBffHandoffError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "OperatorBffHandoffError";
  }
}

function getServerConfig(): OperatorBffServerConfig {
  const baseUrl =
    process.env.OPERATOR_BFF_INTERNAL_BASE_URL ??
    process.env.OPERATOR_BFF_PUBLIC_BASE_URL ??
    (process.env.NODE_ENV === "development" ? LOCAL_OPERATOR_BFF_URL : "");
  const authSecret = process.env.OPERATOR_BFF_AUTH_SECRET ?? "";
  const operatorId = process.env.OPERATOR_CONSOLE_BFF_OPERATOR_ID ?? "";
  const configuredRole = process.env.OPERATOR_CONSOLE_BFF_OPERATOR_ROLE ?? "operator";

  if (!baseUrl) {
    throw new OperatorBffHandoffError(
      "operator_bff_base_url_missing",
      "Operator BFF internal base URL is not configured.",
    );
  }
  if (authSecret.length < 32) {
    throw new OperatorBffHandoffError(
      "operator_bff_auth_not_configured",
      "Operator BFF auth secret must be at least 32 characters.",
    );
  }
  if (!operatorId) {
    throw new OperatorBffHandoffError(
      "operator_bff_operator_identity_missing",
      "Operator console BFF operator identity is not configured.",
    );
  }
  if (configuredRole !== "operator" && configuredRole !== "admin") {
    throw new OperatorBffHandoffError(
      "operator_bff_operator_role_invalid",
      "Operator console BFF role must be operator or admin.",
    );
  }

  return {
    baseUrl: baseUrl.replace(/\/$/, ""),
    authSecret,
    operatorId,
    role: configuredRole,
  };
}

function encodeJson(value: unknown): string {
  return Buffer.from(JSON.stringify(value), "utf8").toString("base64url");
}

function mintOperatorToken(config: OperatorBffServerConfig): string {
  const header = encodeJson({ alg: "HS256", typ: "JWT" });
  const payload = encodeJson({
    sub: config.operatorId,
    role: config.role,
    exp: Math.floor(Date.now() / 1000) + TOKEN_TTL_SECONDS,
  });
  const signature = createHmac("sha256", config.authSecret)
    .update(`${header}.${payload}`)
    .digest("base64url");
  return `${header}.${payload}.${signature}`;
}

function authenticatedHeaders(config: OperatorBffServerConfig): HeadersInit {
  return {
    accept: "application/json",
    authorization: `Bearer ${mintOperatorToken(config)}`,
  };
}

export async function startReadinessWorkflow(): Promise<Response> {
  const config = getServerConfig();
  return fetch(`${config.baseUrl}/v1/workflows/readiness-snapshots`, {
    method: "POST",
    cache: "no-store",
    headers: {
      ...authenticatedHeaders(config),
      "idempotency-key": `console-${randomUUID()}`,
    },
    signal: AbortSignal.timeout(8_000),
  });
}

export async function streamWorkflowEvents(
  workflowId: string,
  signal: AbortSignal,
): Promise<Response> {
  if (!/^[0-9a-f-]{36}$/i.test(workflowId)) {
    throw new OperatorBffHandoffError(
      "operator_bff_workflow_id_invalid",
      "Workflow id is invalid.",
    );
  }
  const config = getServerConfig();
  return fetch(`${config.baseUrl}/v1/workflows/${workflowId}/events`, {
    method: "GET",
    cache: "no-store",
    headers: {
      ...authenticatedHeaders(config),
      accept: "text/event-stream",
    },
    signal,
  });
}
