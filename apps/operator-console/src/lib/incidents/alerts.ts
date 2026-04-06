interface ResponseLike {
  ok: boolean;
  status: number;
  json: () => Promise<unknown>;
}

type FetchLike = (url: string, init: RequestInit) => Promise<ResponseLike>;

interface JsonRecord {
  [key: string]: unknown;
}

export interface IncidentAlertFieldError {
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
  fieldErrors?: IncidentAlertFieldError[];
}

export class IncidentAlertClientError extends Error {
  readonly status: number;
  readonly errorCode: string;
  readonly reasonCode: string;
  readonly action: string;
  readonly endpoint: string;
  readonly timestampUtc: string;
  readonly occurredAt?: string;
  readonly source?: string;
  readonly correlationId?: string;
  readonly fieldErrors: IncidentAlertFieldError[];

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
    this.name = "IncidentAlertClientError";
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

export type IncidentAlertDataState = "ready" | "empty";
export type IncidentAlertSeverity = "warning" | "critical";
export type IncidentAlertStatus = "pending" | "delivered" | "failed";
export type IncidentAlertChannel = "pagerduty" | "slack" | "email";
export type IncidentAlertOutcome = "delivered" | "failed";
export type IncidentAlertDependencyState =
  | "healthy"
  | "dependency_unavailable"
  | "stale_evidence";

export interface IncidentAlertDeliveryAttempt {
  attemptNumber: number;
  channel: IncidentAlertChannel;
  outcome: IncidentAlertOutcome;
  reasonCode: string;
  attemptedAt: string;
  deliveredAt?: string;
  failedAt?: string;
}

export interface IncidentAlertRecord {
  alertId: string;
  severity: IncidentAlertSeverity;
  impactedSubsystem: string;
  cause: string;
  recommendedNextAction: string;
  evidenceLink: string;
  issuedAt: string;
  correlationId: string;
  reasonCode: string;
  status: IncidentAlertStatus;
  deliveredAt?: string;
  failedAt?: string;
  attempts: IncidentAlertDeliveryAttempt[];
}

export interface IncidentAlertsQueryResult {
  status: "accepted";
  action: string;
  actorId: string;
  role: string;
  correlationId: string;
  timestampUtc: string;
  source: string;
  reasonCode: string;
  dataState: IncidentAlertDataState;
  recommendedNextAction: string;
  alerts: IncidentAlertRecord[];
}

export interface QueryIncidentAlertsInput {
  baseUrl: string;
  limit?: number;
  dependencyState?: IncidentAlertDependencyState;
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
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: `missing required response field '${key}'`,
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
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: `invalid timestamp response field '${key}'`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return new Date(parsed).toISOString();
}

function parseFieldErrors(payload: unknown): IncidentAlertFieldError[] {
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
): IncidentAlertClientError {
  if (!isRecord(payload)) {
    return new IncidentAlertClientError({
      status,
      errorCode: "incident_alert_unknown_error",
      reasonCode: "incident_alert_unknown_error",
      message: "Incident alert request failed with a non-object error payload.",
      action: fallbackAction,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const reasonCode =
    optionalString(payload, "reason_code") ??
    optionalString(payload, "error_code") ??
    "incident_alert_unknown_error";

  return new IncidentAlertClientError({
    status,
    errorCode: optionalString(payload, "error_code") ?? reasonCode,
    reasonCode,
    message:
      optionalString(payload, "message") ??
      "Incident alert request failed without explicit message.",
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
    throw new IncidentAlertClientError({
      status: response.status,
      errorCode: "incident_alert_response_parse_failed",
      reasonCode: "incident_alert_response_parse_failed",
      message:
        error instanceof Error
          ? error.message
          : "Incident alert response payload is not valid JSON.",
      action: "incident_alerts_query",
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
    throw new IncidentAlertClientError({
      status: 500,
      errorCode: "incident_alert_fetch_unavailable",
      reasonCode: "incident_alert_fetch_unavailable",
      message: "Fetch implementation is unavailable in this runtime.",
      action: "incident_alerts_query",
      endpoint: "runtime",
      timestampUtc: new Date().toISOString(),
    });
  }
  return fetch as unknown as FetchLike;
}

function validateEvidenceLink(
  value: string,
  endpoint: string,
  action: string,
): string {
  const normalized = value.trim();
  if (!normalized.startsWith("https://") && !normalized.startsWith("http://")) {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: "evidence_link must be an absolute http(s) URL",
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return normalized;
}

function parseAttempt(
  payload: unknown,
  endpoint: string,
): IncidentAlertDeliveryAttempt {
  if (!isRecord(payload)) {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: "alert delivery attempt entry must be an object",
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const channel = requiredString(payload, "channel", endpoint, "incident_alerts_query");
  if (channel !== "pagerduty" && channel !== "slack" && channel !== "email") {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: `unsupported delivery channel '${channel}'`,
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const outcome = requiredString(payload, "outcome", endpoint, "incident_alerts_query");
  if (outcome !== "delivered" && outcome !== "failed") {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: `unsupported delivery outcome '${outcome}'`,
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const attemptNumber = optionalNumber(payload, "attempt_number");
  if (
    attemptNumber === undefined ||
    !Number.isInteger(attemptNumber) ||
    attemptNumber < 1
  ) {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: "attempt_number must be a positive numeric value",
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const deliveredAtRaw = optionalString(payload, "delivered_at");
  const failedAtRaw = optionalString(payload, "failed_at");
  let deliveredAt: string | undefined;
  let failedAt: string | undefined;
  if (outcome === "delivered") {
    if (!deliveredAtRaw || failedAtRaw) {
      throw new IncidentAlertClientError({
        status: 502,
        errorCode: "incident_alert_contract_mismatch",
        reasonCode: "incident_alert_contract_mismatch",
        message: "delivered attempt requires delivered_at and no failed_at",
        action: "incident_alerts_query",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
    deliveredAt = requiredTimestamp(
      payload,
      "delivered_at",
      endpoint,
      "incident_alerts_query",
    );
  } else {
    if (!failedAtRaw || deliveredAtRaw) {
      throw new IncidentAlertClientError({
        status: 502,
        errorCode: "incident_alert_contract_mismatch",
        reasonCode: "incident_alert_contract_mismatch",
        message: "failed attempt requires failed_at and no delivered_at",
        action: "incident_alerts_query",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
    failedAt = requiredTimestamp(
      payload,
      "failed_at",
      endpoint,
      "incident_alerts_query",
    );
  }

  return {
    attemptNumber,
    channel: channel as IncidentAlertChannel,
    outcome: outcome as IncidentAlertOutcome,
    reasonCode: requiredString(payload, "reason_code", endpoint, "incident_alerts_query"),
    attemptedAt: requiredTimestamp(payload, "attempted_at", endpoint, "incident_alerts_query"),
    deliveredAt,
    failedAt,
  };
}

function parseAlert(
  payload: unknown,
  endpoint: string,
): IncidentAlertRecord {
  if (!isRecord(payload)) {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: "alert entry must be an object",
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const severity = requiredString(payload, "severity", endpoint, "incident_alerts_query");
  if (severity !== "warning" && severity !== "critical") {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: `unsupported alert severity '${severity}'`,
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const status = requiredString(payload, "status", endpoint, "incident_alerts_query");
  if (status !== "pending" && status !== "delivered" && status !== "failed") {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: `unsupported alert status '${status}'`,
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const attemptsRaw = payload.attempts;
  if (!Array.isArray(attemptsRaw)) {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: "attempts must be an array",
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const deliveredAtRaw = optionalString(payload, "delivered_at");
  const failedAtRaw = optionalString(payload, "failed_at");
  let deliveredAt: string | undefined;
  let failedAt: string | undefined;
  if (status === "pending") {
    if (deliveredAtRaw || failedAtRaw) {
      throw new IncidentAlertClientError({
        status: 502,
        errorCode: "incident_alert_contract_mismatch",
        reasonCode: "incident_alert_contract_mismatch",
        message: "alert status 'pending' must not include delivered_at or failed_at",
        action: "incident_alerts_query",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
  } else if (status === "delivered") {
    if (!deliveredAtRaw || failedAtRaw) {
      throw new IncidentAlertClientError({
        status: 502,
        errorCode: "incident_alert_contract_mismatch",
        reasonCode: "incident_alert_contract_mismatch",
        message: "alert status 'delivered' requires delivered_at and no failed_at",
        action: "incident_alerts_query",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
    deliveredAt = requiredTimestamp(
      payload,
      "delivered_at",
      endpoint,
      "incident_alerts_query",
    );
  } else {
    if (!failedAtRaw || deliveredAtRaw) {
      throw new IncidentAlertClientError({
        status: 502,
        errorCode: "incident_alert_contract_mismatch",
        reasonCode: "incident_alert_contract_mismatch",
        message: "alert status 'failed' requires failed_at and no delivered_at",
        action: "incident_alerts_query",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
    failedAt = requiredTimestamp(
      payload,
      "failed_at",
      endpoint,
      "incident_alerts_query",
    );
  }

  return {
    alertId: requiredString(payload, "alert_id", endpoint, "incident_alerts_query"),
    severity: severity as IncidentAlertSeverity,
    impactedSubsystem: requiredString(
      payload,
      "impacted_subsystem",
      endpoint,
      "incident_alerts_query",
    ),
    cause: requiredString(payload, "cause", endpoint, "incident_alerts_query"),
    recommendedNextAction: requiredString(
      payload,
      "recommended_next_action",
      endpoint,
      "incident_alerts_query",
    ),
    evidenceLink: validateEvidenceLink(
      requiredString(payload, "evidence_link", endpoint, "incident_alerts_query"),
      endpoint,
      "incident_alerts_query",
    ),
    issuedAt: requiredTimestamp(payload, "issued_at", endpoint, "incident_alerts_query"),
    correlationId: requiredString(payload, "correlation_id", endpoint, "incident_alerts_query"),
    reasonCode: requiredString(payload, "reason_code", endpoint, "incident_alerts_query"),
    status: status as IncidentAlertStatus,
    deliveredAt,
    failedAt,
    attempts: attemptsRaw.map((entry) => parseAttempt(entry, endpoint)),
  };
}

function parseIncidentAlertsPayload(
  payload: unknown,
  endpoint: string,
): IncidentAlertsQueryResult {
  if (!isRecord(payload)) {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: "Expected object payload for incident alerts query response.",
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const status = requiredString(payload, "status", endpoint, "incident_alerts_query");
  if (status !== "accepted") {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: `unexpected incident alert status '${status}'`,
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const dataState = requiredString(payload, "data_state", endpoint, "incident_alerts_query");
  if (dataState !== "ready" && dataState !== "empty") {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: `unsupported incident alert data_state '${dataState}'`,
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const alertsRaw = payload.alerts;
  if (!Array.isArray(alertsRaw)) {
    throw new IncidentAlertClientError({
      status: 502,
      errorCode: "incident_alert_contract_mismatch",
      reasonCode: "incident_alert_contract_mismatch",
      message: "alerts must be an array",
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    status: "accepted",
    action: requiredString(payload, "action", endpoint, "incident_alerts_query"),
    actorId: requiredString(payload, "actor_id", endpoint, "incident_alerts_query"),
    role: requiredString(payload, "role", endpoint, "incident_alerts_query"),
    correlationId: requiredString(payload, "correlation_id", endpoint, "incident_alerts_query"),
    timestampUtc: requiredTimestamp(payload, "timestamp_utc", endpoint, "incident_alerts_query"),
    source: requiredString(payload, "source", endpoint, "incident_alerts_query"),
    reasonCode: requiredString(payload, "reason_code", endpoint, "incident_alerts_query"),
    dataState: dataState as IncidentAlertDataState,
    recommendedNextAction: requiredString(
      payload,
      "recommended_next_action",
      endpoint,
      "incident_alerts_query",
    ),
    alerts: alertsRaw.map((entry) => parseAlert(entry, endpoint)),
  };
}

export async function queryIncidentAlerts({
  baseUrl,
  limit,
  dependencyState,
  fetchImpl,
}: QueryIncidentAlertsInput): Promise<IncidentAlertsQueryResult> {
  const resolvedBaseUrl = stripTrailingSlash(baseUrl.trim());
  const endpointPath = "/control/incidents/alerts";
  const request = resolveFetch(fetchImpl);

  const query = new URLSearchParams();
  if (limit !== undefined) {
    if (!Number.isInteger(limit) || limit < 1 || limit > 200) {
      throw new IncidentAlertClientError({
        status: 400,
        errorCode: "alert_invalid_payload",
        reasonCode: "alert_invalid_payload",
        message: "limit must be an integer between 1 and 200",
        action: "incident_alerts_query",
        endpoint: endpointPath,
        timestampUtc: new Date().toISOString(),
        fieldErrors: [
          {
            field: "limit",
            code: "alert_invalid_payload",
            message: "limit must be an integer between 1 and 200",
          },
        ],
      });
    }
    query.set("limit", limit.toString());
  }
  if (dependencyState) {
    query.set("dependency_state", dependencyState);
  }

  const endpoint = query.size > 0 ? `${endpointPath}?${query.toString()}` : endpointPath;
  try {
    const response = await request(`${resolvedBaseUrl}${endpoint}`, {
      method: "GET",
      credentials: "include",
    });
    const payload = await parsePayload(response, endpoint);
    if (!response.ok) {
      throw parseErrorPayload(payload, response.status, endpoint, "incident_alerts_query");
    }
    return parseIncidentAlertsPayload(payload, endpoint);
  } catch (error) {
    if (error instanceof IncidentAlertClientError) {
      throw error;
    }
    throw new IncidentAlertClientError({
      status: 500,
      errorCode: "incident_alert_request_failed",
      reasonCode: "incident_alert_request_failed",
      message:
        error instanceof Error
          ? error.message
          : "Incident alert request failed before receiving a response.",
      action: "incident_alerts_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
}
