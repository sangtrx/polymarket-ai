interface ResponseLike {
  ok: boolean;
  status: number;
  json: () => Promise<unknown>;
}

type FetchLike = (url: string, init: RequestInit) => Promise<ResponseLike>;

interface JsonRecord {
  [key: string]: unknown;
}

export interface AttributionFieldError {
  field: string;
  code: string;
  message: string;
}

interface ErrorShape {
  status: number;
  errorCode: string;
  message: string;
  action: string;
  endpoint: string;
  timestampUtc: string;
  correlationId?: string;
  fieldErrors?: AttributionFieldError[];
}

export class AttributionClientError extends Error {
  readonly status: number;
  readonly errorCode: string;
  readonly action: string;
  readonly endpoint: string;
  readonly timestampUtc: string;
  readonly correlationId?: string;
  readonly fieldErrors: AttributionFieldError[];

  constructor({
    status,
    errorCode,
    message,
    action,
    endpoint,
    timestampUtc,
    correlationId,
    fieldErrors = [],
  }: ErrorShape) {
    super(message);
    this.name = "AttributionClientError";
    this.status = status;
    this.errorCode = errorCode;
    this.action = action;
    this.endpoint = endpoint;
    this.timestampUtc = timestampUtc;
    this.correlationId = correlationId;
    this.fieldErrors = fieldErrors;
  }
}

export type AttributionPeriod = "1h" | "24h" | "30d";
export type AttributionDataState = "ready" | "empty";
export type AttributionDependencyState =
  | "healthy"
  | "projection_unavailable"
  | "stale_source"
  | "reconciliation_unavailable";

export interface AttributionBreakdownRow {
  marketId: string;
  alphaId: string;
  period: AttributionPeriod;
  periodStartUtc: string;
  periodEndUtc: string;
  realizedPnlUsd: number;
  unrealizedPnlUsd: number;
  grossPnlUsd: number;
  netPnlUsd: number;
  feesUsd: number;
  rebatesUsd: number;
  incentivesUsd: number;
  netCostImpactUsd: number;
  asOfUtc: string;
  source: string;
  reasonCode: string;
  correlationId: string;
  snapshotId?: string;
  runId?: string;
}

export interface AttributionQueryResult {
  status: "accepted";
  action: string;
  actorId: string;
  role: string;
  correlationId: string;
  timestampUtc: string;
  period: AttributionPeriod;
  startInclusiveUtc: string;
  endExclusiveUtc: string;
  asOfUtc: string;
  source: string;
  reasonCode: string;
  dataState: AttributionDataState;
  recommendedNextAction: string;
  rows: AttributionBreakdownRow[];
}

export interface QueryAttributionInput {
  baseUrl: string;
  period?: AttributionPeriod;
  marketId?: string;
  alphaId?: string;
  asOfUtc?: string;
  dependencyState?: AttributionDependencyState;
  fetchImpl?: FetchLike;
}

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
    throw new AttributionClientError({
      status: 502,
      errorCode: "attribution_contract_mismatch",
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
    throw new AttributionClientError({
      status: 502,
      errorCode: "attribution_contract_mismatch",
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
  const resolved = requiredString(record, key, endpoint, action);
  const parsed = Date.parse(resolved);
  if (Number.isNaN(parsed)) {
    throw new AttributionClientError({
      status: 502,
      errorCode: "attribution_contract_mismatch",
      message: `invalid timestamp response field '${key}'`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return new Date(parsed).toISOString();
}

function parseFieldErrors(payload: unknown): AttributionFieldError[] {
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
): AttributionClientError {
  if (!isRecord(payload)) {
    return new AttributionClientError({
      status,
      errorCode: "attribution_unknown_error",
      message: "Attribution request failed with a non-object error payload.",
      action: fallbackAction,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return new AttributionClientError({
    status,
    errorCode: optionalString(payload, "error_code") ?? "attribution_unknown_error",
    message:
      optionalString(payload, "message") ??
      "Attribution request failed without explicit message.",
    action: optionalString(payload, "action") ?? fallbackAction,
    endpoint: optionalString(payload, "endpoint") ?? endpoint,
    timestampUtc: normalizeTimestamp(optionalString(payload, "timestamp_utc")),
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
    throw new AttributionClientError({
      status: response.status,
      errorCode: "attribution_response_parse_failed",
      message:
        error instanceof Error
          ? error.message
          : "Attribution response payload is not valid JSON.",
      action: "attribution_response_parse",
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
    throw new AttributionClientError({
      status: 500,
      errorCode: "attribution_fetch_unavailable",
      message: "Fetch implementation is unavailable in this runtime.",
      action: "attribution_query",
      endpoint: "runtime",
      timestampUtc: new Date().toISOString(),
    });
  }
  return fetch as unknown as FetchLike;
}

function parseAttributionPayload(
  payload: unknown,
  endpoint: string,
): AttributionQueryResult {
  if (!isRecord(payload)) {
    throw new AttributionClientError({
      status: 502,
      errorCode: "attribution_contract_mismatch",
      message: "Expected object payload for attribution query response.",
      action: "attribution_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const status = requiredString(payload, "status", endpoint, "attribution_query");
  if (status !== "accepted") {
    throw new AttributionClientError({
      status: 502,
      errorCode: "attribution_contract_mismatch",
      message: `unexpected attribution status '${status}'`,
      action: "attribution_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const period = requiredString(payload, "period", endpoint, "attribution_query");
  if (period !== "1h" && period !== "24h" && period !== "30d") {
    throw new AttributionClientError({
      status: 502,
      errorCode: "attribution_contract_mismatch",
      message: `unexpected attribution period '${period}'`,
      action: "attribution_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const dataState = requiredString(payload, "data_state", endpoint, "attribution_query");
  if (dataState !== "ready" && dataState !== "empty") {
    throw new AttributionClientError({
      status: 502,
      errorCode: "attribution_contract_mismatch",
      message: `unexpected attribution data_state '${dataState}'`,
      action: "attribution_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const rowsRaw = payload.rows;
  if (!Array.isArray(rowsRaw)) {
    throw new AttributionClientError({
      status: 502,
      errorCode: "attribution_contract_mismatch",
      message: "rows must be an array",
      action: "attribution_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const rows = rowsRaw.map((row, index) => {
    if (!isRecord(row)) {
      throw new AttributionClientError({
        status: 502,
        errorCode: "attribution_contract_mismatch",
        message: `row at index ${index} must be an object`,
        action: "attribution_query",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }

    const rowPeriod = requiredString(row, "period", endpoint, "attribution_query");
    if (rowPeriod !== "1h" && rowPeriod !== "24h" && rowPeriod !== "30d") {
      throw new AttributionClientError({
        status: 502,
        errorCode: "attribution_contract_mismatch",
        message: `unexpected row period '${rowPeriod}'`,
        action: "attribution_query",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }

    return {
      marketId: requiredString(row, "market_id", endpoint, "attribution_query"),
      alphaId: requiredString(row, "alpha_id", endpoint, "attribution_query"),
      period: rowPeriod,
      periodStartUtc: requiredTimestamp(
        row,
        "period_start_utc",
        endpoint,
        "attribution_query",
      ),
      periodEndUtc: requiredTimestamp(row, "period_end_utc", endpoint, "attribution_query"),
      realizedPnlUsd: requiredNumber(row, "realized_pnl_usd", endpoint, "attribution_query"),
      unrealizedPnlUsd: requiredNumber(
        row,
        "unrealized_pnl_usd",
        endpoint,
        "attribution_query",
      ),
      grossPnlUsd: requiredNumber(row, "gross_pnl_usd", endpoint, "attribution_query"),
      netPnlUsd: requiredNumber(row, "net_pnl_usd", endpoint, "attribution_query"),
      feesUsd: requiredNumber(row, "fees_usd", endpoint, "attribution_query"),
      rebatesUsd: requiredNumber(row, "rebates_usd", endpoint, "attribution_query"),
      incentivesUsd: requiredNumber(row, "incentives_usd", endpoint, "attribution_query"),
      netCostImpactUsd: requiredNumber(
        row,
        "net_cost_impact_usd",
        endpoint,
        "attribution_query",
      ),
      asOfUtc: requiredTimestamp(row, "as_of_utc", endpoint, "attribution_query"),
      source: requiredString(row, "source", endpoint, "attribution_query"),
      reasonCode: requiredString(row, "reason_code", endpoint, "attribution_query"),
      correlationId: requiredString(row, "correlation_id", endpoint, "attribution_query"),
      snapshotId: optionalString(row, "snapshot_id"),
      runId: optionalString(row, "run_id"),
    } satisfies AttributionBreakdownRow;
  });

  return {
    status: "accepted",
    action: requiredString(payload, "action", endpoint, "attribution_query"),
    actorId: requiredString(payload, "actor_id", endpoint, "attribution_query"),
    role: requiredString(payload, "role", endpoint, "attribution_query"),
    correlationId: requiredString(payload, "correlation_id", endpoint, "attribution_query"),
    timestampUtc: requiredTimestamp(payload, "timestamp_utc", endpoint, "attribution_query"),
    period,
    startInclusiveUtc: requiredTimestamp(
      payload,
      "start_inclusive_utc",
      endpoint,
      "attribution_query",
    ),
    endExclusiveUtc: requiredTimestamp(
      payload,
      "end_exclusive_utc",
      endpoint,
      "attribution_query",
    ),
    asOfUtc: requiredTimestamp(payload, "as_of_utc", endpoint, "attribution_query"),
    source: requiredString(payload, "source", endpoint, "attribution_query"),
    reasonCode: requiredString(payload, "reason_code", endpoint, "attribution_query"),
    dataState,
    recommendedNextAction: requiredString(
      payload,
      "recommended_next_action",
      endpoint,
      "attribution_query",
    ),
    rows,
  };
}

export async function queryCostAwareAttribution(
  input: QueryAttributionInput,
): Promise<AttributionQueryResult> {
  const period = input.period ?? "24h";
  if (period !== "1h" && period !== "24h" && period !== "30d") {
    throw new AttributionClientError({
      status: 400,
      errorCode: "attribution_invalid_period",
      message: "period must be one of: 1h, 24h, 30d",
      action: "attribution_query",
      endpoint: "/control/portfolio/attribution",
      timestampUtc: new Date().toISOString(),
      fieldErrors: [
        {
          field: "period",
          code: "attribution_invalid_payload",
          message: "period must be one of: 1h, 24h, 30d",
        },
      ],
    });
  }

  const baseUrl = stripTrailingSlash(input.baseUrl.trim());
  if (!baseUrl) {
    throw new AttributionClientError({
      status: 500,
      errorCode: "attribution_base_url_missing",
      message:
        "Operator console API base URL is missing. Set NEXT_PUBLIC_OPERATOR_CONSOLE_API_BASE_URL.",
      action: "attribution_query",
      endpoint: "/control/portfolio/attribution",
      timestampUtc: new Date().toISOString(),
    });
  }

  const query = new URLSearchParams();
  query.set("period", period);
  if (input.marketId?.trim()) {
    query.set("market_id", input.marketId.trim());
  }
  if (input.alphaId?.trim()) {
    query.set("alpha_id", input.alphaId.trim());
  }
  if (input.asOfUtc?.trim()) {
    query.set("as_of_utc", input.asOfUtc.trim());
  }
  if (input.dependencyState) {
    query.set("dependency_state", input.dependencyState);
  }

  const endpoint = `/control/portfolio/attribution?${query.toString()}`;
  const request = resolveFetch(input.fetchImpl);

  try {
    const response = await request(`${baseUrl}${endpoint}`, {
      method: "GET",
    });
    const payload = await parsePayload(response, endpoint);
    if (!response.ok) {
      throw parseErrorPayload(payload, response.status, endpoint, "attribution_query");
    }
    return parseAttributionPayload(payload, endpoint);
  } catch (error) {
    if (error instanceof AttributionClientError) {
      throw error;
    }
    throw new AttributionClientError({
      status: 500,
      errorCode: "attribution_request_failed",
      message:
        error instanceof Error
          ? error.message
          : "Attribution query failed before receiving a response.",
      action: "attribution_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
}
