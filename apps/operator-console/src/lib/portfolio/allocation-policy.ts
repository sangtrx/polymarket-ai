interface ResponseLike {
  ok: boolean;
  status: number;
  json: () => Promise<unknown>;
}

type FetchLike = (url: string, init: RequestInit) => Promise<ResponseLike>;

interface JsonRecord {
  [key: string]: unknown;
}

export interface AllocationPolicyFieldError {
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
  fieldErrors?: AllocationPolicyFieldError[];
}

export class AllocationPolicyClientError extends Error {
  readonly status: number;
  readonly errorCode: string;
  readonly action: string;
  readonly endpoint: string;
  readonly timestampUtc: string;
  readonly correlationId?: string;
  readonly fieldErrors: AllocationPolicyFieldError[];

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
    this.name = "AllocationPolicyClientError";
    this.status = status;
    this.errorCode = errorCode;
    this.action = action;
    this.endpoint = endpoint;
    this.timestampUtc = timestampUtc;
    this.correlationId = correlationId;
    this.fieldErrors = fieldErrors;
  }
}

export interface AllocationPolicyDecision {
  status: "accepted" | "pending" | "denied";
  policyKey: string;
  version: number;
  approvalStatus: string;
  actorId: string;
  role: string;
  reasonCode: string;
  correlationId: string;
  timestampUtc: string;
  approvalReference?: string;
  errorCode?: string;
  message?: string;
}

export interface RebalanceRecommendationDecision {
  status: "accepted" | "pending" | "denied";
  recommendationId: string;
  policyKey: string;
  policyVersion: number;
  recommendationStatus: string;
  approvalStatus: string;
  actionType: string;
  rationale: string;
  recommendedNextAction: string;
  actorId: string;
  role: string;
  reasonCode: string;
  correlationId: string;
  createdAtUtc: string;
  timestampUtc: string;
  approvalReference?: string;
  errorCode?: string;
  message?: string;
}

export interface PendingRebalanceRecommendationItem {
  recommendationId: string;
  policyKey: string;
  policyVersion: number;
  recommendationStatus: string;
  approvalStatus: string;
  actionType: string;
  rationale: string;
  recommendedNextAction: string;
  actorId: string;
  reasonCode: string;
  correlationId: string;
  createdAtUtc: string;
  updatedAtUtc: string;
  approvalReference?: string;
}

export interface PendingRebalanceRecommendationsDecision {
  status: "accepted";
  action: string;
  actorId: string;
  role: string;
  correlationId: string;
  timestampUtc: string;
  pendingRecommendations: PendingRebalanceRecommendationItem[];
}

export interface UpsertAllocationPolicyInput {
  baseUrl: string;
  policyKey: string;
  version: number;
  portfolioScopeId: string;
  targetExposurePctNav: number;
  targetRelativeAlphaWeight: number;
  exposureDriftThresholdPct?: number;
  relativeAlphaDriftThresholdPct?: number;
  advancedParameters?: Record<string, unknown>;
  approvalRequestId?: string;
  fetchImpl?: FetchLike;
}

export interface EvaluateRebalanceInput {
  baseUrl: string;
  policyKey: string;
  exposureDriftPct: number;
  relativeAlphaDriftPct: number;
  observedAtUtc?: string;
  staleAfterSeconds?: number;
  requireExecution?: boolean;
  approvalRequestId?: string;
  fetchImpl?: FetchLike;
}

export interface ExecuteRebalanceRecommendationInput {
  baseUrl: string;
  recommendationId: string;
  executedAtUtc?: string;
  approvalRequestId?: string;
  fetchImpl?: FetchLike;
}

export interface ListPendingRebalanceRecommendationsInput {
  baseUrl: string;
  policyKey?: string;
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

function resolveFetch(fetchImpl: FetchLike | undefined): FetchLike {
  if (fetchImpl) {
    return fetchImpl;
  }
  if (typeof fetch !== "function") {
    throw new AllocationPolicyClientError({
      status: 500,
      errorCode: "allocation_policy_fetch_unavailable",
      message: "Fetch implementation is unavailable in this runtime.",
      action: "allocation_policy_request",
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
    throw new AllocationPolicyClientError({
      status: response.status,
      errorCode: "allocation_policy_response_parse_failed",
      message:
        error instanceof Error
          ? error.message
          : "Allocation policy response payload is not valid JSON.",
      action: "allocation_policy_response_parse",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
}

function requiredString(
  record: JsonRecord,
  key: string,
  endpoint: string,
  action: string,
): string {
  const resolved = optionalString(record, key);
  if (!resolved) {
    throw new AllocationPolicyClientError({
      status: 502,
      errorCode: "allocation_policy_contract_mismatch",
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
    throw new AllocationPolicyClientError({
      status: 502,
      errorCode: "allocation_policy_contract_mismatch",
      message: `missing required numeric response field '${key}'`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return resolved;
}

function parseFieldErrors(payload: unknown): AllocationPolicyFieldError[] {
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
): AllocationPolicyClientError {
  if (!isRecord(payload)) {
    return new AllocationPolicyClientError({
      status,
      errorCode: "allocation_policy_unknown_error",
      message:
        "Allocation policy request failed with a non-object error payload.",
      action: fallbackAction,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return new AllocationPolicyClientError({
    status,
    errorCode:
      optionalString(payload, "error_code") ?? "allocation_policy_unknown_error",
    message:
      optionalString(payload, "message") ??
      "Allocation policy request failed without explicit message.",
    action: optionalString(payload, "action") ?? fallbackAction,
    endpoint: optionalString(payload, "endpoint") ?? endpoint,
    timestampUtc: normalizeTimestamp(optionalString(payload, "timestamp_utc")),
    correlationId: optionalString(payload, "correlation_id"),
    fieldErrors: parseFieldErrors(payload.field_errors),
  });
}

function parseAllocationPolicyDecisionPayload(
  payload: unknown,
  endpoint: string,
): AllocationPolicyDecision {
  if (!isRecord(payload)) {
    throw new AllocationPolicyClientError({
      status: 502,
      errorCode: "allocation_policy_contract_mismatch",
      message: "Expected object payload for allocation policy decision response.",
      action: "allocation_policy_update",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const statusValue = requiredString(
    payload,
    "status",
    endpoint,
    "allocation_policy_update",
  );
  if (
    statusValue !== "accepted" &&
    statusValue !== "pending" &&
    statusValue !== "denied"
  ) {
    throw new AllocationPolicyClientError({
      status: 502,
      errorCode: "allocation_policy_contract_mismatch",
      message: `unexpected allocation policy status '${statusValue}'`,
      action: "allocation_policy_update",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    status: statusValue,
    policyKey: requiredString(
      payload,
      "policy_key",
      endpoint,
      "allocation_policy_update",
    ),
    version: requiredNumber(payload, "version", endpoint, "allocation_policy_update"),
    approvalStatus: requiredString(
      payload,
      "approval_status",
      endpoint,
      "allocation_policy_update",
    ),
    actorId: requiredString(payload, "actor_id", endpoint, "allocation_policy_update"),
    role: requiredString(payload, "role", endpoint, "allocation_policy_update"),
    reasonCode: requiredString(
      payload,
      "reason_code",
      endpoint,
      "allocation_policy_update",
    ),
    correlationId: requiredString(
      payload,
      "correlation_id",
      endpoint,
      "allocation_policy_update",
    ),
    timestampUtc: normalizeTimestamp(optionalString(payload, "timestamp_utc")),
    approvalReference: optionalString(payload, "approval_reference"),
    errorCode: optionalString(payload, "error_code"),
    message: optionalString(payload, "message"),
  };
}

function parseRebalanceDecisionPayload(
  payload: unknown,
  endpoint: string,
): RebalanceRecommendationDecision {
  if (!isRecord(payload)) {
    throw new AllocationPolicyClientError({
      status: 502,
      errorCode: "rebalance_contract_mismatch",
      message: "Expected object payload for rebalance recommendation response.",
      action: "rebalance_recommendation_evaluate",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const statusValue = requiredString(
    payload,
    "status",
    endpoint,
    "rebalance_recommendation_evaluate",
  );
  if (
    statusValue !== "accepted" &&
    statusValue !== "pending" &&
    statusValue !== "denied"
  ) {
    throw new AllocationPolicyClientError({
      status: 502,
      errorCode: "rebalance_contract_mismatch",
      message: `unexpected rebalance status '${statusValue}'`,
      action: "rebalance_recommendation_evaluate",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    status: statusValue,
    recommendationId: requiredString(
      payload,
      "recommendation_id",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    policyKey: requiredString(
      payload,
      "policy_key",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    policyVersion: requiredNumber(
      payload,
      "policy_version",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    recommendationStatus: requiredString(
      payload,
      "recommendation_status",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    approvalStatus: requiredString(
      payload,
      "approval_status",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    actionType: requiredString(
      payload,
      "action_type",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    rationale: requiredString(
      payload,
      "rationale",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    recommendedNextAction: requiredString(
      payload,
      "recommended_next_action",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    actorId: requiredString(
      payload,
      "actor_id",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    role: requiredString(
      payload,
      "role",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    reasonCode: requiredString(
      payload,
      "reason_code",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    correlationId: requiredString(
      payload,
      "correlation_id",
      endpoint,
      "rebalance_recommendation_evaluate",
    ),
    createdAtUtc: normalizeTimestamp(optionalString(payload, "created_at_utc")),
    timestampUtc: normalizeTimestamp(optionalString(payload, "timestamp_utc")),
    approvalReference: optionalString(payload, "approval_reference"),
    errorCode: optionalString(payload, "error_code"),
    message: optionalString(payload, "message"),
  };
}

function parsePendingRebalanceRecommendationsPayload(
  payload: unknown,
  endpoint: string,
): PendingRebalanceRecommendationsDecision {
  if (!isRecord(payload)) {
    throw new AllocationPolicyClientError({
      status: 502,
      errorCode: "rebalance_contract_mismatch",
      message: "Expected object payload for pending recommendation query.",
      action: "rebalance_pending_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const recommendationsRaw = payload.pending_recommendations;
  if (!Array.isArray(recommendationsRaw)) {
    throw new AllocationPolicyClientError({
      status: 502,
      errorCode: "rebalance_contract_mismatch",
      message: "pending_recommendations must be an array.",
      action: "rebalance_pending_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const pendingRecommendations = recommendationsRaw.map((item, index) => {
    if (!isRecord(item)) {
      throw new AllocationPolicyClientError({
        status: 502,
        errorCode: "rebalance_contract_mismatch",
        message: `pending recommendation at index ${index} must be an object.`,
        action: "rebalance_pending_query",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
    return {
      recommendationId: requiredString(
        item,
        "recommendation_id",
        endpoint,
        "rebalance_pending_query",
      ),
      policyKey: requiredString(item, "policy_key", endpoint, "rebalance_pending_query"),
      policyVersion: requiredNumber(
        item,
        "policy_version",
        endpoint,
        "rebalance_pending_query",
      ),
      recommendationStatus: requiredString(
        item,
        "recommendation_status",
        endpoint,
        "rebalance_pending_query",
      ),
      approvalStatus: requiredString(
        item,
        "approval_status",
        endpoint,
        "rebalance_pending_query",
      ),
      actionType: requiredString(item, "action_type", endpoint, "rebalance_pending_query"),
      rationale: requiredString(item, "rationale", endpoint, "rebalance_pending_query"),
      recommendedNextAction: requiredString(
        item,
        "recommended_next_action",
        endpoint,
        "rebalance_pending_query",
      ),
      actorId: requiredString(item, "actor_id", endpoint, "rebalance_pending_query"),
      reasonCode: requiredString(item, "reason_code", endpoint, "rebalance_pending_query"),
      correlationId: requiredString(
        item,
        "correlation_id",
        endpoint,
        "rebalance_pending_query",
      ),
      createdAtUtc: normalizeTimestamp(optionalString(item, "created_at_utc")),
      updatedAtUtc: normalizeTimestamp(optionalString(item, "updated_at_utc")),
      approvalReference: optionalString(item, "approval_reference"),
    };
  });

  return {
    status: "accepted",
    action: requiredString(payload, "action", endpoint, "rebalance_pending_query"),
    actorId: requiredString(payload, "actor_id", endpoint, "rebalance_pending_query"),
    role: requiredString(payload, "role", endpoint, "rebalance_pending_query"),
    correlationId: requiredString(
      payload,
      "correlation_id",
      endpoint,
      "rebalance_pending_query",
    ),
    timestampUtc: normalizeTimestamp(optionalString(payload, "timestamp_utc")),
    pendingRecommendations,
  };
}

async function performRequest<T>({
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
  parseSuccess: (payload: unknown, endpoint: string) => T;
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

    return parseSuccess(payload, endpoint);
  } catch (error) {
    if (error instanceof AllocationPolicyClientError) {
      throw error;
    }
    throw new AllocationPolicyClientError({
      status: 500,
      errorCode: "allocation_policy_request_failed",
      message:
        error instanceof Error
          ? error.message
          : "Allocation policy request failed before receiving a response.",
      action: fallbackAction,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
}

export async function upsertAllocationPolicy(
  input: UpsertAllocationPolicyInput,
): Promise<AllocationPolicyDecision> {
  const policyKey = input.policyKey.trim();
  const endpoint = `/control/allocation-policies/${encodeURIComponent(policyKey)}`;
  return performRequest({
    baseUrl: input.baseUrl,
    endpoint,
    method: "POST",
    fallbackAction: "allocation_policy_update",
    fetchImpl: input.fetchImpl,
    body: JSON.stringify({
      version: input.version,
      portfolio_scope_id: input.portfolioScopeId,
      target_exposure_pct_nav: input.targetExposurePctNav,
      target_relative_alpha_weight: input.targetRelativeAlphaWeight,
      exposure_drift_threshold_pct: input.exposureDriftThresholdPct,
      relative_alpha_drift_threshold_pct: input.relativeAlphaDriftThresholdPct,
      advanced_parameters: input.advancedParameters ?? {},
      approval_request_id: input.approvalRequestId,
    }),
    parseSuccess: parseAllocationPolicyDecisionPayload,
  });
}

export async function evaluateRebalanceRecommendation(
  input: EvaluateRebalanceInput,
): Promise<RebalanceRecommendationDecision> {
  return performRequest({
    baseUrl: input.baseUrl,
    endpoint: "/control/rebalance",
    method: "POST",
    fallbackAction: "rebalance_recommendation_evaluate",
    fetchImpl: input.fetchImpl,
    body: JSON.stringify({
      policy_key: input.policyKey,
      exposure_drift_pct: input.exposureDriftPct,
      relative_alpha_drift_pct: input.relativeAlphaDriftPct,
      observed_at_utc: input.observedAtUtc,
      stale_after_seconds: input.staleAfterSeconds,
      require_execution: input.requireExecution ?? false,
      approval_request_id: input.approvalRequestId,
    }),
    parseSuccess: parseRebalanceDecisionPayload,
  });
}

export async function executeRebalanceRecommendation(
  input: ExecuteRebalanceRecommendationInput,
): Promise<RebalanceRecommendationDecision> {
  const recommendationId = input.recommendationId.trim();
  const endpoint = `/control/rebalance/recommendations/${encodeURIComponent(recommendationId)}/execute`;
  return performRequest({
    baseUrl: input.baseUrl,
    endpoint,
    method: "POST",
    fallbackAction: "rebalance_recommendation_execute",
    fetchImpl: input.fetchImpl,
    body: JSON.stringify({
      executed_at_utc: input.executedAtUtc,
      approval_request_id: input.approvalRequestId,
    }),
    parseSuccess: parseRebalanceDecisionPayload,
  });
}

export async function listPendingRebalanceRecommendations(
  input: ListPendingRebalanceRecommendationsInput,
): Promise<PendingRebalanceRecommendationsDecision> {
  const query = input.policyKey
    ? `?policy_key=${encodeURIComponent(input.policyKey.trim())}`
    : "";
  return performRequest({
    baseUrl: input.baseUrl,
    endpoint: `/control/rebalance/recommendations/pending${query}`,
    method: "GET",
    fallbackAction: "rebalance_pending_query",
    fetchImpl: input.fetchImpl,
    parseSuccess: parsePendingRebalanceRecommendationsPayload,
  });
}
