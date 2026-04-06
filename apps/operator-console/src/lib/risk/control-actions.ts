export type EmergencyControlAction = "pause" | "reduce-only" | "cancel-all";

interface ResponseLike {
  ok: boolean;
  status: number;
  json: () => Promise<unknown>;
}

type FetchLike = (url: string, init: RequestInit) => Promise<ResponseLike>;

export interface EmergencyControlDecision {
  status: "accepted";
  actionId: string;
  action: string;
  source: string;
  triggerSource: string;
  resultingMode: string;
  reasonCode: string;
  correlationId: string;
  timestampUtc: string;
  auditReference?: string;
}

interface EmergencyControlRequestInput {
  baseUrl: string;
  action: EmergencyControlAction;
  auditReference?: string;
  fetchImpl?: FetchLike;
}

interface EmergencyControlActionQueryInput {
  baseUrl: string;
  actionId: string;
  fetchImpl?: FetchLike;
}

interface ErrorShape {
  status: number;
  errorCode: string;
  message: string;
  action: string;
  correlationId?: string;
  timestampUtc: string;
  endpoint: string;
}

const ACTION_ENDPOINTS: Record<EmergencyControlAction, string> = {
  pause: "/control/emergency/pause",
  "reduce-only": "/control/emergency/reduce-only",
  "cancel-all": "/control/emergency/cancel-all",
};

const ACTION_ERROR_KEYS: Record<EmergencyControlAction, string> = {
  pause: "emergency_control_pause",
  "reduce-only": "emergency_control_reduce_only",
  "cancel-all": "emergency_control_cancel_all",
};
const ACTION_ID_PATTERN = /^[a-z0-9._:-]{1,180}$/i;

function stripTrailingSlash(url: string): string {
  return url.endsWith("/") ? url.slice(0, -1) : url;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function optionalString(
  record: Record<string, unknown>,
  key: string,
): string | undefined {
  const candidate = record[key];
  if (typeof candidate !== "string") {
    return undefined;
  }
  const trimmed = candidate.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function requiredString(
  record: Record<string, unknown>,
  key: string,
  context: string,
): string {
  const resolved = optionalString(record, key);
  if (!resolved) {
    throw new EmergencyControlClientError({
      status: 502,
      errorCode: "emergency_control_contract_mismatch",
      message: `missing required response field '${key}' in ${context}`,
      action: "emergency_control_contract_validation",
      endpoint: context,
      timestampUtc: new Date().toISOString(),
    });
  }
  return resolved;
}

function normalizeTimestamp(value: string | undefined): string {
  if (value) {
    const parsed = Date.parse(value);
    if (!Number.isNaN(parsed)) {
      return new Date(parsed).toISOString();
    }
  }
  return new Date().toISOString();
}

function resolveFetch(fetchImpl: FetchLike | undefined): FetchLike {
  if (fetchImpl) {
    return fetchImpl;
  }
  if (typeof fetch !== "function") {
    throw new EmergencyControlClientError({
      status: 500,
      errorCode: "emergency_control_fetch_unavailable",
      message: "Fetch implementation is unavailable in this runtime.",
      action: "emergency_control_request",
      endpoint: "runtime",
      timestampUtc: new Date().toISOString(),
    });
  }
  return fetch as unknown as FetchLike;
}

async function parsePayload(
  response: ResponseLike,
  endpoint: string,
): Promise<unknown> {
  try {
    return await response.json();
  } catch (error) {
    throw new EmergencyControlClientError({
      status: response.status,
      errorCode: "emergency_control_response_parse_failed",
      message:
        error instanceof Error
          ? error.message
          : "Emergency control response payload is not valid JSON.",
      action: "emergency_control_response_parse",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
}

function parseDecisionPayload(
  payload: unknown,
  endpoint: string,
  status: number,
): EmergencyControlDecision {
  if (!isRecord(payload)) {
    throw new EmergencyControlClientError({
      status,
      errorCode: "emergency_control_contract_mismatch",
      message: "Expected object payload for emergency control decision response.",
      action: "emergency_control_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const resolvedStatus = requiredString(payload, "status", endpoint);
  if (resolvedStatus !== "accepted") {
    throw new EmergencyControlClientError({
      status,
      errorCode: "emergency_control_unexpected_status",
      message: `Expected status='accepted' but received '${resolvedStatus}'.`,
      action: "emergency_control_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    status: "accepted",
    actionId: requiredString(payload, "action_id", endpoint),
    action: requiredString(payload, "action", endpoint),
    source: requiredString(payload, "source", endpoint),
    triggerSource: requiredString(payload, "trigger_source", endpoint),
    resultingMode: requiredString(payload, "resulting_mode", endpoint),
    reasonCode: requiredString(payload, "reason_code", endpoint),
    correlationId: requiredString(payload, "correlation_id", endpoint),
    timestampUtc: normalizeTimestamp(optionalString(payload, "timestamp_utc")),
    auditReference: optionalString(payload, "audit_reference"),
  };
}

function parseErrorPayload(
  payload: unknown,
  status: number,
  endpoint: string,
  fallbackAction: string,
): EmergencyControlClientError {
  if (!isRecord(payload)) {
    return new EmergencyControlClientError({
      status,
      errorCode: "emergency_control_unknown_error",
      message: "Emergency control request failed with a non-object error payload.",
      action: fallbackAction,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return new EmergencyControlClientError({
    status,
    errorCode:
      optionalString(payload, "error_code") ?? "emergency_control_unknown_error",
    message:
      optionalString(payload, "message") ??
      "Emergency control request failed without explicit message.",
    action: optionalString(payload, "action") ?? fallbackAction,
    correlationId: optionalString(payload, "correlation_id"),
    endpoint: optionalString(payload, "endpoint") ?? endpoint,
    timestampUtc: normalizeTimestamp(optionalString(payload, "timestamp_utc")),
  });
}

async function performEmergencyControlRequest({
  baseUrl,
  endpoint,
  method,
  body,
  fallbackAction,
  fetchImpl,
}: {
  baseUrl: string;
  endpoint: string;
  method: "GET" | "POST";
  body?: string;
  fallbackAction: string;
  fetchImpl?: FetchLike;
}): Promise<EmergencyControlDecision> {
  const resolvedBaseUrl = stripTrailingSlash(baseUrl.trim());
  const resolvedEndpoint = `${resolvedBaseUrl}${endpoint}`;
  const requester = resolveFetch(fetchImpl);

  try {
    const response = await requester(resolvedEndpoint, {
      method,
      headers: body
        ? {
            "content-type": "application/json",
          }
        : undefined,
      body,
    });
    const payload = await parsePayload(response, endpoint);

    if (!response.ok) {
      throw parseErrorPayload(payload, response.status, endpoint, fallbackAction);
    }

    return parseDecisionPayload(payload, endpoint, response.status);
  } catch (error) {
    if (error instanceof EmergencyControlClientError) {
      throw error;
    }
    throw new EmergencyControlClientError({
      status: 500,
      errorCode: "emergency_control_request_failed",
      message:
        error instanceof Error
          ? error.message
          : "Emergency control request failed before receiving a response.",
      action: fallbackAction,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
}

export async function invokeEmergencyControlAction({
  baseUrl,
  action,
  auditReference,
  fetchImpl,
}: EmergencyControlRequestInput): Promise<EmergencyControlDecision> {
  const endpoint = ACTION_ENDPOINTS[action];
  return performEmergencyControlRequest({
    baseUrl,
    endpoint,
    method: "POST",
    body: JSON.stringify({
      audit_reference: auditReference,
    }),
    fallbackAction: ACTION_ERROR_KEYS[action],
    fetchImpl,
  });
}

export async function getEmergencyControlActionResult({
  baseUrl,
  actionId: requestedActionId,
  fetchImpl,
}: EmergencyControlActionQueryInput): Promise<EmergencyControlDecision> {
  const actionId = requestedActionId.trim();
  if (!actionId || !ACTION_ID_PATTERN.test(actionId)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "emergency_control_invalid_action_id",
      message: "Action result lookup requires a non-empty action_id.",
      action: "emergency_control_action_query",
      endpoint: "/control/emergency/actions/{action_id}",
      timestampUtc: new Date().toISOString(),
    });
  }

  return performEmergencyControlRequest({
    baseUrl,
    endpoint: `/control/emergency/actions/${encodeURIComponent(actionId)}`,
    method: "GET",
    fallbackAction: "emergency_control_action_query",
    fetchImpl,
  });
}

export function normalizeEmergencyAction(
  action: string,
): EmergencyControlAction | undefined {
  const normalized = action.trim().toLowerCase();
  if (normalized === "pause") {
    return "pause";
  }
  if (normalized === "reduce_only" || normalized === "reduce-only") {
    return "reduce-only";
  }
  if (normalized === "cancel_all" || normalized === "cancel-all") {
    return "cancel-all";
  }
  return undefined;
}

export class EmergencyControlClientError extends Error {
  readonly status: number;
  readonly errorCode: string;
  readonly action: string;
  readonly correlationId?: string;
  readonly timestampUtc: string;
  readonly endpoint: string;

  constructor({
    status,
    errorCode,
    message,
    action,
    correlationId,
    timestampUtc,
    endpoint,
  }: ErrorShape) {
    super(message);
    this.name = "EmergencyControlClientError";
    this.status = status;
    this.errorCode = errorCode;
    this.action = action;
    this.correlationId = correlationId;
    this.timestampUtc = timestampUtc;
    this.endpoint = endpoint;
  }
}
