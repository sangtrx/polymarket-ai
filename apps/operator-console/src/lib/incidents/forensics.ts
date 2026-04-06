interface ResponseLike {
  ok: boolean;
  status: number;
  json: () => Promise<unknown>;
}

type FetchLike = (url: string, init: RequestInit) => Promise<ResponseLike>;

interface JsonRecord {
  [key: string]: unknown;
}

export interface IncidentFieldError {
  field: string;
  code: string;
  message: string;
}

interface ErrorShape {
  status: number;
  errorCode: string;
  reasonCode: string;
  message: string;
  action: string;
  endpoint: string;
  timestampUtc: string;
  occurredAt?: string;
  source?: string;
  correlationId?: string;
  fieldErrors?: IncidentFieldError[];
}

export class IncidentForensicsClientError extends Error {
  readonly status: number;
  readonly errorCode: string;
  readonly reasonCode: string;
  readonly action: string;
  readonly endpoint: string;
  readonly timestampUtc: string;
  readonly occurredAt?: string;
  readonly source?: string;
  readonly correlationId?: string;
  readonly fieldErrors: IncidentFieldError[];

  constructor({
    status,
    errorCode,
    reasonCode,
    message,
    action,
    endpoint,
    timestampUtc,
    occurredAt,
    source,
    correlationId,
    fieldErrors = [],
  }: ErrorShape) {
    super(message);
    this.name = "IncidentForensicsClientError";
    this.status = status;
    this.errorCode = errorCode;
    this.reasonCode = reasonCode;
    this.action = action;
    this.endpoint = endpoint;
    this.timestampUtc = timestampUtc;
    this.occurredAt = occurredAt;
    this.source = source;
    this.correlationId = correlationId;
    this.fieldErrors = fieldErrors;
  }
}

export type IncidentDataState = "ready" | "empty";
export type IncidentSeverity = "normal" | "warning" | "critical" | "degraded";
export type IncidentTimelineStage = "signal" | "order" | "fill" | "pnl" | "risk_action";

export interface IncidentTimelineEvent {
  eventId: string;
  occurredAt: string;
  stage: IncidentTimelineStage;
  source: string;
  reasonCode: string;
  correlationId: string;
  summary: string;
  recommendedNextAction: string;
  severity: IncidentSeverity | "degraded";
  marketId?: string;
  orderId?: string;
  alphaId?: string;
  actorId?: string;
  runId?: string;
  snapshotId?: string;
}

export interface IncidentFilterSummary {
  marketId?: string;
  orderId?: string;
  alphaId?: string;
  actorId?: string;
}

export interface IncidentCausalFlowSummary {
  trigger: string;
  context: string;
  action: string;
  verification: string;
}

export interface IncidentForensicsQueryResult {
  status: "accepted";
  action: string;
  actorId: string;
  role: string;
  correlationId: string;
  timestampUtc: string;
  startInclusiveUtc: string;
  endExclusiveUtc: string;
  source: string;
  reasonCode: string;
  dataState: IncidentDataState;
  severity: IncidentSeverity;
  queryLatencyMs: number;
  p95LatencyTargetMs: number;
  recommendedNextAction: string;
  filters: IncidentFilterSummary;
  causalFlow: IncidentCausalFlowSummary;
  events: IncidentTimelineEvent[];
}

export interface QueryIncidentForensicsInput {
  baseUrl: string;
  marketId?: string;
  orderId?: string;
  alphaId?: string;
  actorId?: string;
  startTs?: string;
  endTs?: string;
  fetchImpl?: FetchLike;
}

const CANONICAL_IDENTIFIER_PATTERN = /^[a-z0-9._:-]{3,120}$/i;
const STAGE_VALUES: ReadonlySet<string> = new Set([
  "signal",
  "order",
  "fill",
  "pnl",
  "risk_action",
]);

const SEVERITY_VALUES: ReadonlySet<string> = new Set([
  "normal",
  "warning",
  "critical",
  "degraded",
]);

function stripTrailingSlash(url: string): string {
  return url.endsWith("/") ? url.slice(0, -1) : url;
}

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null;
}

function optionalString(record: JsonRecord, key: string): string | undefined {
  const candidate = record[key];
  if (typeof candidate !== "string") {
    return undefined;
  }
  const trimmed = candidate.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function optionalNumber(record: JsonRecord, key: string): number | undefined {
  const candidate = record[key];
  if (typeof candidate !== "number" || !Number.isFinite(candidate)) {
    return undefined;
  }
  return candidate;
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

function requiredString(
  record: JsonRecord,
  key: string,
  endpoint: string,
  action: string,
): string {
  const resolved = optionalString(record, key);
  if (!resolved) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: `missing required response field '${key}'`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return resolved;
}

function requiredNumber(
  record: JsonRecord,
  key: string,
  endpoint: string,
  action: string,
): number {
  const resolved = optionalNumber(record, key);
  if (resolved === undefined) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: `missing required numeric response field '${key}'`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return resolved;
}

function requiredTimestamp(
  record: JsonRecord,
  key: string,
  endpoint: string,
  action: string,
): string {
  const value = requiredString(record, key, endpoint, action);
  const parsed = Date.parse(value);
  if (Number.isNaN(parsed)) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: `invalid timestamp response field '${key}'`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return new Date(parsed).toISOString();
}

function parseFieldErrors(payload: unknown): IncidentFieldError[] {
  if (!Array.isArray(payload)) {
    return [];
  }
  return payload.flatMap((entry) => {
    if (!isRecord(entry)) {
      return [];
    }
    const field = optionalString(entry, "field");
    const code = optionalString(entry, "code");
    const message = optionalString(entry, "message");
    if (!field || !code || !message) {
      return [];
    }
    return [{ field, code, message }];
  });
}

function parseErrorPayload(
  payload: unknown,
  status: number,
  endpoint: string,
  fallbackAction: string,
): IncidentForensicsClientError {
  if (!isRecord(payload)) {
    return new IncidentForensicsClientError({
      status,
      errorCode: "incident_unknown_error",
      reasonCode: "incident_unknown_error",
      message: "Incident request failed with a non-object error payload.",
      action: fallbackAction,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const reasonCode =
    optionalString(payload, "reason_code") ??
    optionalString(payload, "error_code") ??
    "incident_unknown_error";

  return new IncidentForensicsClientError({
    status,
    errorCode: optionalString(payload, "error_code") ?? reasonCode,
    reasonCode,
    message:
      optionalString(payload, "message") ??
      "Incident request failed without explicit message.",
    action: optionalString(payload, "action") ?? fallbackAction,
    endpoint: optionalString(payload, "endpoint") ?? endpoint,
    timestampUtc: normalizeTimestamp(optionalString(payload, "timestamp_utc")),
    occurredAt: normalizeTimestamp(optionalString(payload, "occurred_at")),
    source: optionalString(payload, "source"),
    correlationId: optionalString(payload, "correlation_id"),
    fieldErrors: parseFieldErrors(payload.field_errors),
  });
}

async function parsePayload(
  response: ResponseLike,
  endpoint: string,
): Promise<unknown> {
  try {
    return await response.json();
  } catch (error) {
    throw new IncidentForensicsClientError({
      status: response.status,
      errorCode: "incident_response_parse_failed",
      reasonCode: "incident_response_parse_failed",
      message:
        error instanceof Error
          ? error.message
          : "Incident response payload is not valid JSON.",
      action: "incident_response_parse",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
}

function resolveFetch(fetchImpl: FetchLike | undefined): FetchLike {
  if (fetchImpl) {
    return fetchImpl;
  }
  if (typeof fetch !== "function") {
    throw new IncidentForensicsClientError({
      status: 500,
      errorCode: "incident_fetch_unavailable",
      reasonCode: "incident_fetch_unavailable",
      message: "Fetch implementation is unavailable in this runtime.",
      action: "incident_query",
      endpoint: "runtime",
      timestampUtc: new Date().toISOString(),
    });
  }
  return fetch as unknown as FetchLike;
}

function normalizeCanonicalIdentifier(
  field: string,
  value: string | undefined,
): string | undefined {
  if (!value) {
    return undefined;
  }
  const normalized = value.trim().toLowerCase();
  if (!normalized) {
    return undefined;
  }
  if (!CANONICAL_IDENTIFIER_PATTERN.test(normalized)) {
    throw new IncidentForensicsClientError({
      status: 400,
      errorCode: "incident_invalid_payload",
      reasonCode: "incident_invalid_payload",
      message: `${field} must contain 3-120 canonical characters`,
      action: "incident_query",
      endpoint: "/control/incidents/forensics",
      timestampUtc: new Date().toISOString(),
      fieldErrors: [
        {
          field,
          code: "incident_invalid_payload",
          message: `${field} must contain 3-120 canonical characters`,
        },
      ],
    });
  }
  return normalized;
}

function resolveQueryWindow(
  startTs: string | undefined,
  endTs: string | undefined,
): { startTs: string; endTs: string } {
  const normalizedStart = startTs?.trim();
  const normalizedEnd = endTs?.trim();

  if ((normalizedStart && !normalizedEnd) || (!normalizedStart && normalizedEnd)) {
    throw new IncidentForensicsClientError({
      status: 400,
      errorCode: "incident_invalid_payload",
      reasonCode: "incident_invalid_payload",
      message: "start_ts and end_ts must be provided together",
      action: "incident_query",
      endpoint: "/control/incidents/forensics",
      timestampUtc: new Date().toISOString(),
      fieldErrors: [
        {
          field: "start_ts",
          code: "incident_invalid_payload",
          message: "start_ts and end_ts must be provided together",
        },
        {
          field: "end_ts",
          code: "incident_invalid_payload",
          message: "start_ts and end_ts must be provided together",
        },
      ],
    });
  }

  if (!normalizedStart && !normalizedEnd) {
    const end = new Date();
    const start = new Date(end.getTime() - 24 * 60 * 60 * 1000);
    return { startTs: start.toISOString(), endTs: end.toISOString() };
  }

  const parsedStart = Date.parse(normalizedStart!);
  const parsedEnd = Date.parse(normalizedEnd!);
  if (Number.isNaN(parsedStart) || Number.isNaN(parsedEnd)) {
    throw new IncidentForensicsClientError({
      status: 400,
      errorCode: "incident_invalid_payload",
      reasonCode: "incident_invalid_payload",
      message: "start_ts and end_ts must be valid ISO-8601 UTC timestamps",
      action: "incident_query",
      endpoint: "/control/incidents/forensics",
      timestampUtc: new Date().toISOString(),
    });
  }
  if (parsedEnd <= parsedStart) {
    throw new IncidentForensicsClientError({
      status: 400,
      errorCode: "incident_invalid_payload",
      reasonCode: "incident_invalid_payload",
      message: "end_ts must be greater than start_ts",
      action: "incident_query",
      endpoint: "/control/incidents/forensics",
      timestampUtc: new Date().toISOString(),
      fieldErrors: [
        {
          field: "end_ts",
          code: "incident_invalid_payload",
          message: "end_ts must be greater than start_ts",
        },
      ],
    });
  }
  return {
    startTs: new Date(parsedStart).toISOString(),
    endTs: new Date(parsedEnd).toISOString(),
  };
}

function parseIncidentFilters(payload: unknown): IncidentFilterSummary {
  if (!isRecord(payload)) {
    return {};
  }
  return {
    marketId: optionalString(payload, "market_id"),
    orderId: optionalString(payload, "order_id"),
    alphaId: optionalString(payload, "alpha_id"),
    actorId: optionalString(payload, "actor_id"),
  };
}

function parseCausalFlow(
  payload: unknown,
  endpoint: string,
): IncidentCausalFlowSummary {
  if (!isRecord(payload)) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: "causal_flow must be an object payload",
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return {
    trigger: requiredString(payload, "trigger", endpoint, "incident_query"),
    context: requiredString(payload, "context", endpoint, "incident_query"),
    action: requiredString(payload, "action", endpoint, "incident_query"),
    verification: requiredString(payload, "verification", endpoint, "incident_query"),
  };
}

function parseTimelineEvent(
  payload: unknown,
  endpoint: string,
): IncidentTimelineEvent {
  if (!isRecord(payload)) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: "timeline event entry must be an object",
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const stage = requiredString(payload, "stage", endpoint, "incident_query");
  if (!STAGE_VALUES.has(stage)) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: `unsupported timeline stage '${stage}'`,
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const severity = requiredString(payload, "severity", endpoint, "incident_query");
  if (!SEVERITY_VALUES.has(severity)) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: `unsupported incident severity '${severity}'`,
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    eventId: requiredString(payload, "event_id", endpoint, "incident_query"),
    occurredAt: requiredTimestamp(payload, "occurred_at", endpoint, "incident_query"),
    stage: stage as IncidentTimelineStage,
    source: requiredString(payload, "source", endpoint, "incident_query"),
    reasonCode: requiredString(payload, "reason_code", endpoint, "incident_query"),
    correlationId: requiredString(payload, "correlation_id", endpoint, "incident_query"),
    summary: requiredString(payload, "summary", endpoint, "incident_query"),
    recommendedNextAction: requiredString(
      payload,
      "recommended_next_action",
      endpoint,
      "incident_query",
    ),
    severity: severity as IncidentSeverity | "degraded",
    marketId: optionalString(payload, "market_id"),
    orderId: optionalString(payload, "order_id"),
    alphaId: optionalString(payload, "alpha_id"),
    actorId: optionalString(payload, "actor_id"),
    runId: optionalString(payload, "run_id"),
    snapshotId: optionalString(payload, "snapshot_id"),
  };
}

function parseIncidentPayload(
  payload: unknown,
  endpoint: string,
): IncidentForensicsQueryResult {
  if (!isRecord(payload)) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: "Expected object payload for incident query response.",
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const status = requiredString(payload, "status", endpoint, "incident_query");
  if (status !== "accepted") {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: `unexpected incident status '${status}'`,
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const dataState = requiredString(payload, "data_state", endpoint, "incident_query");
  if (dataState !== "ready" && dataState !== "empty") {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: `unsupported incident data_state '${dataState}'`,
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const severity = requiredString(payload, "severity", endpoint, "incident_query");
  if (
    severity !== "normal" &&
    severity !== "warning" &&
    severity !== "critical" &&
    severity !== "degraded"
  ) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: `unsupported incident severity '${severity}'`,
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const rawEvents = payload.events;
  if (!Array.isArray(rawEvents)) {
    throw new IncidentForensicsClientError({
      status: 502,
      errorCode: "incident_contract_mismatch",
      reasonCode: "incident_contract_mismatch",
      message: "events must be an array in incident query response",
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    status: "accepted",
    action: requiredString(payload, "action", endpoint, "incident_query"),
    actorId: requiredString(payload, "actor_id", endpoint, "incident_query"),
    role: requiredString(payload, "role", endpoint, "incident_query"),
    correlationId: requiredString(payload, "correlation_id", endpoint, "incident_query"),
    timestampUtc: requiredTimestamp(payload, "timestamp_utc", endpoint, "incident_query"),
    startInclusiveUtc: requiredTimestamp(
      payload,
      "start_inclusive_utc",
      endpoint,
      "incident_query",
    ),
    endExclusiveUtc: requiredTimestamp(
      payload,
      "end_exclusive_utc",
      endpoint,
      "incident_query",
    ),
    source: requiredString(payload, "source", endpoint, "incident_query"),
    reasonCode: requiredString(payload, "reason_code", endpoint, "incident_query"),
    dataState: dataState as IncidentDataState,
    severity: severity as IncidentSeverity,
    queryLatencyMs: requiredNumber(payload, "query_latency_ms", endpoint, "incident_query"),
    p95LatencyTargetMs: requiredNumber(
      payload,
      "p95_latency_target_ms",
      endpoint,
      "incident_query",
    ),
    recommendedNextAction: requiredString(
      payload,
      "recommended_next_action",
      endpoint,
      "incident_query",
    ),
    filters: parseIncidentFilters(payload.filters),
    causalFlow: parseCausalFlow(payload.causal_flow, endpoint),
    events: rawEvents.map((entry) => parseTimelineEvent(entry, endpoint)),
  };
}

export async function queryIncidentForensics({
  baseUrl,
  marketId,
  orderId,
  alphaId,
  actorId,
  startTs,
  endTs,
  fetchImpl,
}: QueryIncidentForensicsInput): Promise<IncidentForensicsQueryResult> {
  const resolvedBaseUrl = stripTrailingSlash(baseUrl.trim());
  const endpoint = "/control/incidents/forensics";
  const requester = resolveFetch(fetchImpl);

  const normalizedMarketId = normalizeCanonicalIdentifier("market_id", marketId);
  const normalizedOrderId = normalizeCanonicalIdentifier("order_id", orderId);
  const normalizedAlphaId = normalizeCanonicalIdentifier("alpha_id", alphaId);
  const normalizedActorId = normalizeCanonicalIdentifier("actor_id", actorId);
  const window = resolveQueryWindow(startTs, endTs);

  const params = new URLSearchParams();
  params.set("start_ts", window.startTs);
  params.set("end_ts", window.endTs);
  if (normalizedMarketId) {
    params.set("market_id", normalizedMarketId);
  }
  if (normalizedOrderId) {
    params.set("order_id", normalizedOrderId);
  }
  if (normalizedAlphaId) {
    params.set("alpha_id", normalizedAlphaId);
  }
  if (normalizedActorId) {
    params.set("actor_id", normalizedActorId);
  }

  const resolvedEndpoint = `${resolvedBaseUrl}${endpoint}?${params.toString()}`;
  try {
    const response = await requester(resolvedEndpoint, {
      method: "GET",
      credentials: "include",
    });
    const payload = await parsePayload(response, endpoint);
    if (!response.ok) {
      throw parseErrorPayload(payload, response.status, endpoint, "incident_query");
    }
    return parseIncidentPayload(payload, endpoint);
  } catch (error) {
    if (error instanceof IncidentForensicsClientError) {
      throw error;
    }
    throw new IncidentForensicsClientError({
      status: 500,
      errorCode: "incident_request_failed",
      reasonCode: "incident_request_failed",
      message:
        error instanceof Error
          ? error.message
          : "Incident query failed before receiving a response.",
      action: "incident_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
}
