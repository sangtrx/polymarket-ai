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

export type RecoveryReadinessStatus = "approved" | "blocked";

export interface RecoveryGateOutcomeEvidence {
  gate: string;
  passed: boolean;
  reasonCode: string;
  trigger: string;
  context: string;
  action: string;
  verification: string;
}

export interface RecoveryReadinessDecision {
  status: "accepted";
  action: string;
  runId: string;
  readinessStatus: RecoveryReadinessStatus;
  reasonCode: string;
  profileKey: string;
  actorId: string;
  actorRole: string;
  correlationId: string;
  requestedAtUtc: string;
  evaluatedAtUtc: string;
  resumedAtUtc?: string;
  reconciliationRunId?: string;
  approvedChecksum?: string;
  computedChecksum?: string;
  failingGateCodes: string[];
  gateOutcomes: RecoveryGateOutcomeEvidence[];
  recommendedNextAction: string;
  auditReference?: string;
  timestampUtc: string;
}

export interface RecoveryResumeDecision {
  status: "accepted";
  action: string;
  runId: string;
  readinessStatus: RecoveryReadinessStatus;
  reasonCode: string;
  verificationReasonCode: string;
  actorId: string;
  actorRole: string;
  correlationId: string;
  resumedAtUtc: string;
  verificationTimestampUtc: string;
  timestampUtc: string;
  auditReference?: string;
}

interface EmergencyControlRequestInput {
  baseUrl: string;
  action: EmergencyControlAction;
  auditReference?: string;
  fetchImpl?: FetchLike;
}

interface RecoveryReadinessRequestInput {
  baseUrl: string;
  profileKey: string;
  reconciliationRunId: string;
  approvedChecksum: string;
  signoffIntent: string;
  auditReference?: string;
  fetchImpl?: FetchLike;
}

interface RecoveryResumeRequestInput {
  baseUrl: string;
  runId: string;
  resumedAtUtc?: string;
  fetchImpl?: FetchLike;
}

interface RecoveryRunQueryInput {
  baseUrl: string;
  runId?: string;
  correlationId?: string;
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
const CHECKSUM_PATTERN = /^[0-9a-f]{64}$/;
const RECOVERY_READINESS_ENDPOINT = "/control/recovery/readiness/evaluate";
const RECOVERY_RESUME_ENDPOINT = "/control/recovery/resume";
const RECOVERY_RUNS_ENDPOINT = "/control/recovery/runs";

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

function requiredBoolean(
  record: Record<string, unknown>,
  key: string,
  context: string,
): boolean {
  const candidate = record[key];
  if (typeof candidate !== "boolean") {
    throw new EmergencyControlClientError({
      status: 502,
      errorCode: "emergency_control_contract_mismatch",
      message: `missing required boolean response field '${key}' in ${context}`,
      action: "emergency_control_contract_validation",
      endpoint: context,
      timestampUtc: new Date().toISOString(),
    });
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

function parseReadinessStatus(
  payload: Record<string, unknown>,
  endpoint: string,
): RecoveryReadinessStatus {
  const readinessStatus = requiredString(payload, "readiness_status", endpoint);
  if (readinessStatus === "approved" || readinessStatus === "blocked") {
    return readinessStatus;
  }
  throw new EmergencyControlClientError({
    status: 502,
    errorCode: "recovery_contract_mismatch",
    message: `Expected readiness_status to be approved|blocked, received '${readinessStatus}'.`,
    action: "recovery_contract_validation",
    endpoint,
    timestampUtc: new Date().toISOString(),
  });
}

function parseRecoveryGateOutcomes(
  payload: Record<string, unknown>,
  endpoint: string,
): RecoveryGateOutcomeEvidence[] {
  const outcomesRaw = payload.gate_outcomes;
  if (!Array.isArray(outcomesRaw)) {
    throw new EmergencyControlClientError({
      status: 502,
      errorCode: "recovery_contract_mismatch",
      message: "Expected gate_outcomes array in recovery readiness response.",
      action: "recovery_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return outcomesRaw.map((candidate, index) => {
    if (!isRecord(candidate)) {
      throw new EmergencyControlClientError({
        status: 502,
        errorCode: "recovery_contract_mismatch",
        message: `Expected object gate_outcomes[${index}]`,
        action: "recovery_contract_validation",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
    return {
      gate: requiredString(candidate, "gate", endpoint),
      passed: requiredBoolean(candidate, "passed", endpoint),
      reasonCode: requiredString(candidate, "reason_code", endpoint),
      trigger: requiredString(candidate, "trigger", endpoint),
      context: requiredString(candidate, "context", endpoint),
      action: requiredString(candidate, "action", endpoint),
      verification: requiredString(candidate, "verification", endpoint),
    };
  });
}

function parseRecoveryReadinessPayload(
  payload: unknown,
  endpoint: string,
  status: number,
): RecoveryReadinessDecision {
  if (!isRecord(payload)) {
    throw new EmergencyControlClientError({
      status,
      errorCode: "recovery_contract_mismatch",
      message: "Expected object payload for recovery readiness response.",
      action: "recovery_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const resolvedStatus = requiredString(payload, "status", endpoint);
  if (resolvedStatus !== "accepted") {
    throw new EmergencyControlClientError({
      status,
      errorCode: "recovery_unexpected_status",
      message: `Expected status='accepted' but received '${resolvedStatus}'.`,
      action: "recovery_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const failingGateCodesRaw = payload.failing_gate_codes;
  const failingGateCodes = Array.isArray(failingGateCodesRaw)
    ? failingGateCodesRaw
        .filter((value): value is string => typeof value === "string")
        .map((value) => value.trim())
        .filter((value) => value.length > 0)
    : [];

  return {
    status: "accepted",
    action: requiredString(payload, "action", endpoint),
    runId: requiredString(payload, "run_id", endpoint),
    readinessStatus: parseReadinessStatus(payload, endpoint),
    reasonCode: requiredString(payload, "reason_code", endpoint),
    profileKey: requiredString(payload, "profile_key", endpoint),
    actorId: requiredString(payload, "actor_id", endpoint),
    actorRole: requiredString(payload, "actor_role", endpoint),
    correlationId: requiredString(payload, "correlation_id", endpoint),
    requestedAtUtc: normalizeTimestamp(
      requiredString(payload, "requested_at_utc", endpoint),
    ),
    evaluatedAtUtc: normalizeTimestamp(
      requiredString(payload, "evaluated_at_utc", endpoint),
    ),
    resumedAtUtc: optionalString(payload, "resumed_at_utc"),
    reconciliationRunId: optionalString(payload, "reconciliation_run_id"),
    approvedChecksum: optionalString(payload, "approved_checksum"),
    computedChecksum: optionalString(payload, "computed_checksum"),
    failingGateCodes,
    gateOutcomes: parseRecoveryGateOutcomes(payload, endpoint),
    recommendedNextAction: requiredString(payload, "recommended_next_action", endpoint),
    auditReference: optionalString(payload, "audit_reference"),
    timestampUtc: normalizeTimestamp(
      requiredString(payload, "timestamp_utc", endpoint),
    ),
  };
}

function parseRecoveryResumePayload(
  payload: unknown,
  endpoint: string,
  status: number,
): RecoveryResumeDecision {
  if (!isRecord(payload)) {
    throw new EmergencyControlClientError({
      status,
      errorCode: "recovery_contract_mismatch",
      message: "Expected object payload for recovery resume response.",
      action: "recovery_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const resolvedStatus = requiredString(payload, "status", endpoint);
  if (resolvedStatus !== "accepted") {
    throw new EmergencyControlClientError({
      status,
      errorCode: "recovery_unexpected_status",
      message: `Expected status='accepted' but received '${resolvedStatus}'.`,
      action: "recovery_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    status: "accepted",
    action: requiredString(payload, "action", endpoint),
    runId: requiredString(payload, "run_id", endpoint),
    readinessStatus: parseReadinessStatus(payload, endpoint),
    reasonCode: requiredString(payload, "reason_code", endpoint),
    verificationReasonCode: requiredString(
      payload,
      "verification_reason_code",
      endpoint,
    ),
    actorId: requiredString(payload, "actor_id", endpoint),
    actorRole: requiredString(payload, "actor_role", endpoint),
    correlationId: requiredString(payload, "correlation_id", endpoint),
    resumedAtUtc: normalizeTimestamp(
      requiredString(payload, "resumed_at_utc", endpoint),
    ),
    verificationTimestampUtc: normalizeTimestamp(
      requiredString(payload, "verification_timestamp_utc", endpoint),
    ),
    timestampUtc: normalizeTimestamp(
      requiredString(payload, "timestamp_utc", endpoint),
    ),
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

async function performRecoveryRequest<T>({
  baseUrl,
  endpoint,
  method,
  body,
  fallbackAction,
  fetchImpl,
  parseSuccess,
}: {
  baseUrl: string;
  endpoint: string;
  method: "GET" | "POST";
  body?: string;
  fallbackAction: string;
  fetchImpl?: FetchLike;
  parseSuccess: (payload: unknown, endpoint: string, status: number) => T;
}): Promise<T> {
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
    return parseSuccess(payload, endpoint, response.status);
  } catch (error) {
    if (error instanceof EmergencyControlClientError) {
      throw error;
    }
    throw new EmergencyControlClientError({
      status: 500,
      errorCode: "recovery_request_failed",
      message:
        error instanceof Error
          ? error.message
          : "Recovery control request failed before receiving a response.",
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

export async function invokeRecoveryReadinessEvaluation({
  baseUrl,
  profileKey,
  reconciliationRunId,
  approvedChecksum,
  signoffIntent,
  auditReference,
  fetchImpl,
}: RecoveryReadinessRequestInput): Promise<RecoveryReadinessDecision> {
  const normalizedProfileKey = profileKey.trim();
  const normalizedReconciliationRunId = reconciliationRunId.trim();
  const normalizedChecksum = approvedChecksum.trim().toLowerCase();
  const normalizedSignoffIntent = signoffIntent.trim();
  if (!normalizedProfileKey) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "recovery_invalid_profile_key",
      message: "Recovery readiness evaluation requires a non-empty profile_key.",
      action: "recovery_readiness_evaluate",
      endpoint: RECOVERY_READINESS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (!normalizedReconciliationRunId) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "recovery_invalid_reconciliation_run_id",
      message:
        "Recovery readiness evaluation requires a non-empty reconciliation_run_id.",
      action: "recovery_readiness_evaluate",
      endpoint: RECOVERY_READINESS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (!CHECKSUM_PATTERN.test(normalizedChecksum)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "recovery_invalid_checksum",
      message:
        "Recovery readiness evaluation requires approved_checksum as lowercase 64-char hex.",
      action: "recovery_readiness_evaluate",
      endpoint: RECOVERY_READINESS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (!normalizedSignoffIntent) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "recovery_signoff_missing",
      message: "Recovery readiness evaluation requires explicit signoff_intent.",
      action: "recovery_readiness_evaluate",
      endpoint: RECOVERY_READINESS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }

  return performRecoveryRequest({
    baseUrl,
    endpoint: RECOVERY_READINESS_ENDPOINT,
    method: "POST",
    body: JSON.stringify({
      profile_key: normalizedProfileKey,
      reconciliation_run_id: normalizedReconciliationRunId,
      approved_checksum: normalizedChecksum,
      signoff_intent: normalizedSignoffIntent,
      audit_reference: auditReference,
    }),
    fallbackAction: "recovery_readiness_evaluate",
    fetchImpl,
    parseSuccess: parseRecoveryReadinessPayload,
  });
}

export async function invokeRecoveryResume({
  baseUrl,
  runId: requestedRunId,
  resumedAtUtc,
  fetchImpl,
}: RecoveryResumeRequestInput): Promise<RecoveryResumeDecision> {
  const runId = requestedRunId.trim();
  if (!runId || !ACTION_ID_PATTERN.test(runId)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "recovery_invalid_run_id",
      message: "Recovery resume requires a non-empty run_id.",
      action: "recovery_resume_execute",
      endpoint: RECOVERY_RESUME_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }

  return performRecoveryRequest({
    baseUrl,
    endpoint: RECOVERY_RESUME_ENDPOINT,
    method: "POST",
    body: JSON.stringify({
      run_id: runId,
      resumed_at_utc: resumedAtUtc,
    }),
    fallbackAction: "recovery_resume_execute",
    fetchImpl,
    parseSuccess: parseRecoveryResumePayload,
  });
}

export async function queryRecoveryGateRun({
  baseUrl,
  runId,
  correlationId,
  fetchImpl,
}: RecoveryRunQueryInput): Promise<RecoveryReadinessDecision> {
  const normalizedRunId = runId?.trim();
  const normalizedCorrelationId = correlationId?.trim();
  if (normalizedRunId) {
    if (!ACTION_ID_PATTERN.test(normalizedRunId)) {
      throw new EmergencyControlClientError({
        status: 400,
        errorCode: "recovery_invalid_run_id",
        message: "Recovery run query requires a valid run_id.",
        action: "recovery_readiness_query",
        endpoint: `${RECOVERY_RUNS_ENDPOINT}/{run_id}`,
        timestampUtc: new Date().toISOString(),
      });
    }
    return performRecoveryRequest({
      baseUrl,
      endpoint: `${RECOVERY_RUNS_ENDPOINT}/${encodeURIComponent(normalizedRunId)}`,
      method: "GET",
      fallbackAction: "recovery_readiness_query",
      fetchImpl,
      parseSuccess: parseRecoveryReadinessPayload,
    });
  }
  if (normalizedCorrelationId) {
    return performRecoveryRequest({
      baseUrl,
      endpoint: `${RECOVERY_RUNS_ENDPOINT}?correlation_id=${encodeURIComponent(
        normalizedCorrelationId,
      )}`,
      method: "GET",
      fallbackAction: "recovery_readiness_query",
      fetchImpl,
      parseSuccess: parseRecoveryReadinessPayload,
    });
  }
  throw new EmergencyControlClientError({
    status: 400,
    errorCode: "recovery_query_missing_selector",
    message: "Recovery run query requires runId or correlationId selector.",
    action: "recovery_readiness_query",
    endpoint: RECOVERY_RUNS_ENDPOINT,
    timestampUtc: new Date().toISOString(),
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
