interface ResponseLike {
  ok: boolean;
  status: number;
  json: () => Promise<unknown>;
}

type FetchLike = (url: string, init: RequestInit) => Promise<ResponseLike>;

interface JsonRecord {
  [key: string]: unknown;
}

export interface GovernanceReadinessFieldError {
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
  fieldErrors?: GovernanceReadinessFieldError[];
}

export class GovernanceReadinessClientError extends Error {
  readonly status: number;
  readonly errorCode: string;
  readonly action: string;
  readonly endpoint: string;
  readonly timestampUtc: string;
  readonly correlationId?: string;
  readonly fieldErrors: GovernanceReadinessFieldError[];

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
    this.name = "GovernanceReadinessClientError";
    this.status = status;
    this.errorCode = errorCode;
    this.action = action;
    this.endpoint = endpoint;
    this.timestampUtc = timestampUtc;
    this.correlationId = correlationId;
    this.fieldErrors = fieldErrors;
  }
}

export type GovernanceLifecycleState =
  | "draft"
  | "shadow"
  | "candidate-live"
  | "production"
  | "deallocated";
export type GovernanceValidationCompleteness = "complete" | "incomplete";
export type GovernanceShadowStability = "stable" | "unstable" | "missing";
export type GovernanceGuardrailState = "healthy" | "breached" | "missing";
export type GovernanceReadinessStatus = "ready" | "blocked";

export interface GovernanceReadinessWindow {
  window: "1h" | "24h" | "30d";
  available: boolean;
  netPnl?: number;
  rollingSharpe?: number;
  rollingHitRate?: number;
  rollingDrawdown?: number;
  stabilityScore?: number;
  boundarySemantics: string;
}

export interface AlphaGovernanceReadinessResult {
  candidateId: string;
  alphaId: string;
  lifecycleState: GovernanceLifecycleState;
  validationCompleteness: GovernanceValidationCompleteness;
  shadowStability: GovernanceShadowStability;
  guardrailState: GovernanceGuardrailState;
  readinessStatus: GovernanceReadinessStatus;
  missingArtifacts: string[];
  recommendedNextAction: string;
  asOfUtc: string;
  source: string;
  reasonCode: string;
  correlationId: string;
  validationRunId?: string;
  guardrailReasonCodes: string[];
  guardrailWindows: GovernanceReadinessWindow[];
}

export interface QueryAlphaGovernanceReadinessInput {
  baseUrl: string;
  candidateId: string;
  alphaId: string;
  limit?: number;
  fetchImpl?: FetchLike;
}

interface PromotionDecisionEvidence {
  decisionId: string;
  candidateId: string;
  validationRunId: string;
  lifecycleAction: string;
  decisionState: string;
  reasonCode: string;
  evidencePacket: JsonRecord;
  missingEvidenceFields: string[];
  correlationId: string;
  decidedAtUtc: string;
}

interface ShadowEvaluationEvidence {
  evaluationId: string;
  candidateId: string;
  validationRunId: string;
  evaluationState: string;
  reasonCode: string;
  simulationOutcomeCount: number;
  correlationId: string;
  startedAtUtc: string;
  completedAtUtc?: string;
}

interface ValidationRunEvidence {
  runId: string;
  candidateId: string;
  runState: string;
  reasonCode: string;
  startedAtUtc: string;
  completedAtUtc?: string;
}

interface ValidationArtifactEvidence {
  artifactId: string;
  runId: string;
  stage: string;
  stageOutcome: string;
  reasonCode: string;
  stageCompletedAtUtc: string;
}

interface AlphaHealthWindowEvidence {
  window: string;
  netPnl: number;
  rollingSharpe: number;
  rollingHitRate: number;
  rollingDrawdown: number;
  stabilityScore: number;
}

interface AlphaHealthMetricEvidence {
  metricId: string;
  alphaId: string;
  reasonCode: string;
  correlationId: string;
  recordedAtUtc: string;
  windows: AlphaHealthWindowEvidence[];
}

interface AlphaThresholdBreachEvidence {
  breachId: string;
  alphaId: string;
  metricKey: string;
  comparator: string;
  observedValue: number;
  thresholdValue: number;
  breachReason: string;
  reasonCode: string;
  correlationId: string;
  breachedAtUtc: string;
}

interface EnvelopeMeta {
  action: string;
  actorId: string;
  role: string;
  correlationId: string;
  timestampUtc: string;
  endpoint: string;
}

interface ReadinessDerivationInput {
  candidateId: string;
  alphaId: string;
  promotionDecisions: PromotionDecisionEvidence[];
  shadowEvaluations: ShadowEvaluationEvidence[];
  validationRuns: ValidationRunEvidence[];
  validationArtifacts: ValidationArtifactEvidence[];
  alphaHealthMetrics: AlphaHealthMetricEvidence[];
  alphaThresholdBreaches: AlphaThresholdBreachEvidence[];
  metadata: EnvelopeMeta[];
}

interface ValidationRunDetailEvidence {
  run: ValidationRunEvidence;
  artifacts: ValidationArtifactEvidence[];
}

const CANONICAL_IDENTIFIER_PATTERN = /^[a-z0-9._:-]{3,120}$/i;
const REQUIRED_VALIDATION_STAGES = [
  "quality",
  "labeling",
  "purged_cv",
  "cpcv",
  "overfit_diagnostics",
] as const;
const REQUIRED_PROMOTION_PACKET_FIELDS = [
  "data_quality_report",
  "purged_cpcv_results",
  "calibration_report",
  "counterfactual_replay_summary",
] as const;
const REQUIRED_FR10_WINDOWS = ["1h", "24h", "30d"] as const;
const DEFAULT_FETCH_LIMIT = 20;
const WINDOW_BOUNDARY_SEMANTICS =
  "Floor metrics breach on strict <, drawdown breaches on strict >, and equality remains on allow-path.";

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
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
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
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
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
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `invalid timestamp response field '${key}'`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return new Date(parsed).toISOString();
}

function requiredRecord(
  record: JsonRecord,
  key: string,
  endpoint: string,
  action: string,
): JsonRecord {
  const candidate = record[key];
  if (!isRecord(candidate)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `response field '${key}' must be an object`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return candidate;
}

function requiredArray(
  record: JsonRecord,
  key: string,
  endpoint: string,
  action: string,
): unknown[] {
  const candidate = record[key];
  if (!Array.isArray(candidate)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `response field '${key}' must be an array`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return candidate;
}

function parseFieldErrors(payload: unknown): GovernanceReadinessFieldError[] {
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
): GovernanceReadinessClientError {
  if (!isRecord(payload)) {
    return new GovernanceReadinessClientError({
      status,
      errorCode: "governance_readiness_unknown_error",
      message: "Governance readiness request failed with a non-object error payload.",
      action: fallbackAction,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const envelopeMeta = isRecord(payload.meta) ? payload.meta : undefined;
  const envelopeError = isRecord(payload.error) ? payload.error : undefined;
  const source = envelopeError ?? payload;

  return new GovernanceReadinessClientError({
    status,
    errorCode:
      optionalString(source, "error_code") ?? "governance_readiness_unknown_error",
    message:
      optionalString(source, "message") ??
      "Governance readiness request failed without explicit message.",
    action: optionalString(envelopeMeta ?? source, "action") ?? fallbackAction,
    endpoint: envelopeMeta
      ? optionalString(envelopeMeta, "endpoint") ?? endpoint
      : endpoint,
    timestampUtc: normalizeTimestamp(
      optionalString(envelopeMeta ?? source, "timestamp_utc"),
    ),
    correlationId: optionalString(envelopeMeta ?? source, "correlation_id"),
    fieldErrors: parseFieldErrors(source.field_errors),
  });
}

async function parsePayload(
  response: ResponseLike,
  endpoint: string,
): Promise<unknown> {
  try {
    return await response.json();
  } catch (error) {
    throw new GovernanceReadinessClientError({
      status: response.status,
      errorCode: "governance_readiness_response_parse_failed",
      message:
        error instanceof Error
          ? error.message
          : "Governance readiness response payload is not valid JSON.",
      action: "governance_readiness_query",
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
    throw new GovernanceReadinessClientError({
      status: 500,
      errorCode: "governance_readiness_fetch_unavailable",
      message: "Fetch implementation is unavailable in this runtime.",
      action: "governance_readiness_query",
      endpoint: "runtime",
      timestampUtc: new Date().toISOString(),
    });
  }
  return fetch as unknown as FetchLike;
}

function resolveLimit(limit: number | undefined): number {
  if (limit === undefined) {
    return DEFAULT_FETCH_LIMIT;
  }
  if (!Number.isInteger(limit) || limit <= 0 || limit > 100) {
    throw new GovernanceReadinessClientError({
      status: 400,
      errorCode: "governance_readiness_invalid_payload",
      message: "limit must be an integer between 1 and 100",
      action: "governance_readiness_query",
      endpoint: "/control/research",
      timestampUtc: new Date().toISOString(),
      fieldErrors: [
        {
          field: "limit",
          code: "governance_readiness_invalid_payload",
          message: "limit must be an integer between 1 and 100",
        },
      ],
    });
  }
  return limit;
}

function normalizeCanonicalIdentifier(field: string, value: string): string {
  const normalized = value.trim().toLowerCase();
  if (!normalized || !CANONICAL_IDENTIFIER_PATTERN.test(normalized)) {
    throw new GovernanceReadinessClientError({
      status: 400,
      errorCode: "governance_readiness_invalid_payload",
      message: `${field} must contain 3-120 canonical characters`,
      action: "governance_readiness_query",
      endpoint: "/control/research",
      timestampUtc: new Date().toISOString(),
      fieldErrors: [
        {
          field,
          code: "governance_readiness_invalid_payload",
          message: `${field} must contain 3-120 canonical characters`,
        },
      ],
    });
  }
  return normalized;
}

function buildEndpoint(
  path: string,
  query: Record<string, string | number | undefined>,
): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined) {
      params.set(key, String(value));
    }
  }
  return `${path}?${params.toString()}`;
}

function parseEnvelopeMeta(
  payload: JsonRecord,
  endpoint: string,
  action: string,
): EnvelopeMeta {
  const meta = requiredRecord(payload, "meta", endpoint, action);
  return {
    action: requiredString(meta, "action", endpoint, action),
    actorId: requiredString(meta, "actor_id", endpoint, action),
    role: requiredString(meta, "role", endpoint, action),
    correlationId: requiredString(meta, "correlation_id", endpoint, action),
    timestampUtc: requiredTimestamp(meta, "timestamp_utc", endpoint, action),
    endpoint: requiredString(meta, "endpoint", endpoint, action),
  };
}

function parseResponseData(
  payload: unknown,
  endpoint: string,
  action: string,
): { data: JsonRecord; meta: EnvelopeMeta } {
  if (!isRecord(payload)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: "Expected object payload for governance readiness response.",
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const meta = parseEnvelopeMeta(payload, endpoint, action);
  const envelopeError = payload.error;
  if (envelopeError !== undefined && envelopeError !== null) {
    throw parseErrorPayload(
      { error: envelopeError, meta: payload.meta },
      502,
      endpoint,
      action,
    );
  }
  if (!isRecord(payload.data)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: "response data payload must be an object",
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
      correlationId: meta.correlationId,
    });
  }
  return { data: payload.data, meta };
}

function parseStringArray(
  payload: unknown,
  fieldName: string,
  endpoint: string,
  action: string,
): string[] {
  if (payload === undefined || payload === null) {
    return [];
  }
  if (!Array.isArray(payload)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `${fieldName} must be an array`,
      action,
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return payload.map((entry, index) => {
    if (typeof entry !== "string" || entry.trim().length === 0) {
      throw new GovernanceReadinessClientError({
        status: 502,
        errorCode: "governance_readiness_contract_mismatch",
        message: `${fieldName}[${index}] must be a non-empty string`,
        action,
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
    return entry.trim();
  });
}

function parsePromotionDecisionItem(
  payload: unknown,
  endpoint: string,
): PromotionDecisionEvidence {
  if (!isRecord(payload)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: "promotion decision entry must be an object",
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    decisionId: requiredString(
      payload,
      "decision_id",
      endpoint,
      "governance_readiness_query",
    ),
    candidateId: requiredString(
      payload,
      "candidate_id",
      endpoint,
      "governance_readiness_query",
    ),
    validationRunId: requiredString(
      payload,
      "validation_run_id",
      endpoint,
      "governance_readiness_query",
    ),
    lifecycleAction: requiredString(
      payload,
      "lifecycle_action",
      endpoint,
      "governance_readiness_query",
    ),
    decisionState: requiredString(
      payload,
      "decision_state",
      endpoint,
      "governance_readiness_query",
    ),
    reasonCode: requiredString(
      payload,
      "reason_code",
      endpoint,
      "governance_readiness_query",
    ),
    evidencePacket: requiredRecord(
      payload,
      "evidence_packet",
      endpoint,
      "governance_readiness_query",
    ),
    missingEvidenceFields: parseStringArray(
      payload.missing_evidence_fields,
      "missing_evidence_fields",
      endpoint,
      "governance_readiness_query",
    ),
    correlationId: requiredString(
      payload,
      "correlation_id",
      endpoint,
      "governance_readiness_query",
    ),
    decidedAtUtc: requiredTimestamp(
      payload,
      "decided_at_utc",
      endpoint,
      "governance_readiness_query",
    ),
  };
}

function parseShadowEvaluationItem(
  payload: unknown,
  endpoint: string,
): ShadowEvaluationEvidence {
  if (!isRecord(payload)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: "shadow evaluation entry must be an object",
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const simulationOutcomes = requiredArray(
    payload,
    "simulation_outcomes",
    endpoint,
    "governance_readiness_query",
  );

  return {
    evaluationId: requiredString(
      payload,
      "evaluation_id",
      endpoint,
      "governance_readiness_query",
    ),
    candidateId: requiredString(
      payload,
      "candidate_id",
      endpoint,
      "governance_readiness_query",
    ),
    validationRunId: requiredString(
      payload,
      "validation_run_id",
      endpoint,
      "governance_readiness_query",
    ),
    evaluationState: requiredString(
      payload,
      "evaluation_state",
      endpoint,
      "governance_readiness_query",
    ),
    reasonCode: requiredString(
      payload,
      "reason_code",
      endpoint,
      "governance_readiness_query",
    ),
    simulationOutcomeCount: simulationOutcomes.length,
    correlationId: requiredString(
      payload,
      "correlation_id",
      endpoint,
      "governance_readiness_query",
    ),
    startedAtUtc: requiredTimestamp(
      payload,
      "started_at_utc",
      endpoint,
      "governance_readiness_query",
    ),
    completedAtUtc: optionalString(payload, "completed_at_utc")
      ? requiredTimestamp(
          payload,
          "completed_at_utc",
          endpoint,
          "governance_readiness_query",
        )
      : undefined,
  };
}

function parseValidationRunItem(
  payload: unknown,
  endpoint: string,
): ValidationRunEvidence {
  if (!isRecord(payload)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: "validation run entry must be an object",
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    runId: requiredString(payload, "run_id", endpoint, "governance_readiness_query"),
    candidateId: requiredString(
      payload,
      "candidate_id",
      endpoint,
      "governance_readiness_query",
    ),
    runState: requiredString(
      payload,
      "run_state",
      endpoint,
      "governance_readiness_query",
    ),
    reasonCode: requiredString(
      payload,
      "reason_code",
      endpoint,
      "governance_readiness_query",
    ),
    startedAtUtc: requiredTimestamp(
      payload,
      "started_at_utc",
      endpoint,
      "governance_readiness_query",
    ),
    completedAtUtc: optionalString(payload, "completed_at_utc")
      ? requiredTimestamp(
          payload,
          "completed_at_utc",
          endpoint,
          "governance_readiness_query",
        )
      : undefined,
  };
}

function parseValidationArtifactItem(
  payload: unknown,
  endpoint: string,
): ValidationArtifactEvidence {
  if (!isRecord(payload)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: "validation artifact entry must be an object",
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  return {
    artifactId: requiredString(
      payload,
      "artifact_id",
      endpoint,
      "governance_readiness_query",
    ),
    runId: requiredString(payload, "run_id", endpoint, "governance_readiness_query"),
    stage: requiredString(payload, "stage", endpoint, "governance_readiness_query"),
    stageOutcome: requiredString(
      payload,
      "stage_outcome",
      endpoint,
      "governance_readiness_query",
    ),
    reasonCode: requiredString(
      payload,
      "reason_code",
      endpoint,
      "governance_readiness_query",
    ),
    stageCompletedAtUtc: requiredTimestamp(
      payload,
      "stage_completed_at_utc",
      endpoint,
      "governance_readiness_query",
    ),
  };
}

function parseAlphaHealthWindowItem(
  payload: unknown,
  endpoint: string,
): AlphaHealthWindowEvidence {
  if (!isRecord(payload)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: "alpha health window entry must be an object",
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return {
    window: requiredString(payload, "window", endpoint, "governance_readiness_query"),
    netPnl: requiredNumber(payload, "net_pnl", endpoint, "governance_readiness_query"),
    rollingSharpe: requiredNumber(
      payload,
      "rolling_sharpe",
      endpoint,
      "governance_readiness_query",
    ),
    rollingHitRate: requiredNumber(
      payload,
      "rolling_hit_rate",
      endpoint,
      "governance_readiness_query",
    ),
    rollingDrawdown: requiredNumber(
      payload,
      "rolling_drawdown",
      endpoint,
      "governance_readiness_query",
    ),
    stabilityScore: requiredNumber(
      payload,
      "stability_score",
      endpoint,
      "governance_readiness_query",
    ),
  };
}

function parseAlphaHealthMetricItem(
  payload: unknown,
  endpoint: string,
): AlphaHealthMetricEvidence {
  if (!isRecord(payload)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: "alpha health metric entry must be an object",
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }

  const windows = requiredArray(
    payload,
    "windows",
    endpoint,
    "governance_readiness_query",
  ).map((entry) => parseAlphaHealthWindowItem(entry, endpoint));

  return {
    metricId: requiredString(
      payload,
      "metric_id",
      endpoint,
      "governance_readiness_query",
    ),
    alphaId: requiredString(payload, "alpha_id", endpoint, "governance_readiness_query"),
    reasonCode: requiredString(
      payload,
      "reason_code",
      endpoint,
      "governance_readiness_query",
    ),
    correlationId: requiredString(
      payload,
      "correlation_id",
      endpoint,
      "governance_readiness_query",
    ),
    recordedAtUtc: requiredTimestamp(
      payload,
      "recorded_at_utc",
      endpoint,
      "governance_readiness_query",
    ),
    windows,
  };
}

function parseAlphaThresholdBreachItem(
  payload: unknown,
  endpoint: string,
): AlphaThresholdBreachEvidence {
  if (!isRecord(payload)) {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: "alpha threshold breach entry must be an object",
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return {
    breachId: requiredString(payload, "breach_id", endpoint, "governance_readiness_query"),
    alphaId: requiredString(payload, "alpha_id", endpoint, "governance_readiness_query"),
    metricKey: requiredString(payload, "metric_key", endpoint, "governance_readiness_query"),
    comparator: requiredString(payload, "comparator", endpoint, "governance_readiness_query"),
    observedValue: requiredNumber(
      payload,
      "observed_value",
      endpoint,
      "governance_readiness_query",
    ),
    thresholdValue: requiredNumber(
      payload,
      "threshold_value",
      endpoint,
      "governance_readiness_query",
    ),
    breachReason: requiredString(
      payload,
      "breach_reason",
      endpoint,
      "governance_readiness_query",
    ),
    reasonCode: requiredString(
      payload,
      "reason_code",
      endpoint,
      "governance_readiness_query",
    ),
    correlationId: requiredString(
      payload,
      "correlation_id",
      endpoint,
      "governance_readiness_query",
    ),
    breachedAtUtc: requiredTimestamp(
      payload,
      "breached_at_utc",
      endpoint,
      "governance_readiness_query",
    ),
  };
}

async function requestEnvelope<T>({
  baseUrl,
  endpoint,
  fetchImpl,
  parseData,
}: {
  baseUrl: string;
  endpoint: string;
  fetchImpl: FetchLike;
  parseData: (data: JsonRecord, endpoint: string) => T;
}): Promise<{ data: T; meta: EnvelopeMeta }> {
  const response = await fetchImpl(`${baseUrl}${endpoint}`, {
    method: "GET",
    credentials: "include",
  });
  const payload = await parsePayload(response, endpoint);
  if (!response.ok) {
    throw parseErrorPayload(
      payload,
      response.status,
      endpoint,
      "governance_readiness_query",
    );
  }
  const parsed = parseResponseData(payload, endpoint, "governance_readiness_query");
  return {
    data: parseData(parsed.data, endpoint),
    meta: parsed.meta,
  };
}

function parsePromotionDecisionsData(
  data: JsonRecord,
  endpoint: string,
): PromotionDecisionEvidence[] {
  const kind = requiredString(data, "kind", endpoint, "governance_readiness_query");
  if (kind !== "decisions") {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `unexpected promotion data kind '${kind}'`,
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const decisions = requiredArray(
    data,
    "decisions",
    endpoint,
    "governance_readiness_query",
  );
  return decisions.map((entry) => parsePromotionDecisionItem(entry, endpoint));
}

function parseShadowEvaluationsData(
  data: JsonRecord,
  endpoint: string,
): ShadowEvaluationEvidence[] {
  const kind = requiredString(data, "kind", endpoint, "governance_readiness_query");
  if (kind !== "evaluations") {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `unexpected shadow-evaluation data kind '${kind}'`,
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const evaluations = requiredArray(
    data,
    "evaluations",
    endpoint,
    "governance_readiness_query",
  );
  return evaluations.map((entry) => parseShadowEvaluationItem(entry, endpoint));
}

function parseValidationRunsData(
  data: JsonRecord,
  endpoint: string,
): ValidationRunEvidence[] {
  const kind = requiredString(data, "kind", endpoint, "governance_readiness_query");
  if (kind !== "runs") {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `unexpected validation-runs data kind '${kind}'`,
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const runs = requiredArray(data, "runs", endpoint, "governance_readiness_query");
  return runs.map((entry) => parseValidationRunItem(entry, endpoint));
}

function parseValidationRunDetailData(
  data: JsonRecord,
  endpoint: string,
): ValidationRunDetailEvidence {
  const kind = requiredString(data, "kind", endpoint, "governance_readiness_query");
  if (kind !== "run_detail") {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `unexpected validation-run detail kind '${kind}'`,
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const runPayload = requiredRecord(data, "run", endpoint, "governance_readiness_query");
  const artifactsPayload = requiredArray(
    data,
    "artifacts",
    endpoint,
    "governance_readiness_query",
  );
  return {
    run: parseValidationRunItem(runPayload, endpoint),
    artifacts: artifactsPayload.map((entry) =>
      parseValidationArtifactItem(entry, endpoint),
    ),
  };
}

function parseAlphaHealthMetricsData(
  data: JsonRecord,
  endpoint: string,
): AlphaHealthMetricEvidence[] {
  const kind = requiredString(data, "kind", endpoint, "governance_readiness_query");
  if (kind !== "metrics") {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `unexpected alpha-health data kind '${kind}'`,
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const metrics = requiredArray(data, "metrics", endpoint, "governance_readiness_query");
  return metrics.map((entry) => parseAlphaHealthMetricItem(entry, endpoint));
}

function parseAlphaThresholdBreachesData(
  data: JsonRecord,
  endpoint: string,
): AlphaThresholdBreachEvidence[] {
  const kind = requiredString(data, "kind", endpoint, "governance_readiness_query");
  if (kind !== "breaches") {
    throw new GovernanceReadinessClientError({
      status: 502,
      errorCode: "governance_readiness_contract_mismatch",
      message: `unexpected alpha-threshold-breach data kind '${kind}'`,
      action: "governance_readiness_query",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const breaches = requiredArray(
    data,
    "breaches",
    endpoint,
    "governance_readiness_query",
  );
  return breaches.map((entry) => parseAlphaThresholdBreachItem(entry, endpoint));
}

function toCanonicalState(value: string): string {
  return value.trim().toLowerCase();
}

function latestIsoTimestamp(values: Array<string | undefined>): string {
  const latest = values.reduce<string | undefined>((current, candidate) => {
    if (!candidate) {
      return current;
    }
    if (!current) {
      return candidate;
    }
    return Date.parse(candidate) > Date.parse(current) ? candidate : current;
  }, undefined);
  return normalizeTimestamp(latest);
}

function selectLatestDecision(
  decisions: PromotionDecisionEvidence[],
): PromotionDecisionEvidence | undefined {
  return [...decisions].sort(
    (left, right) => Date.parse(right.decidedAtUtc) - Date.parse(left.decidedAtUtc),
  )[0];
}

function selectLatestShadowEvaluation(
  evaluations: ShadowEvaluationEvidence[],
): ShadowEvaluationEvidence | undefined {
  return [...evaluations].sort((left, right) => {
    const rightValue = Date.parse(right.completedAtUtc ?? right.startedAtUtc);
    const leftValue = Date.parse(left.completedAtUtc ?? left.startedAtUtc);
    return rightValue - leftValue;
  })[0];
}

function selectLatestValidationRun(
  runs: ValidationRunEvidence[],
): ValidationRunEvidence | undefined {
  return [...runs].sort((left, right) => {
    const rightValue = Date.parse(right.completedAtUtc ?? right.startedAtUtc);
    const leftValue = Date.parse(left.completedAtUtc ?? left.startedAtUtc);
    return rightValue - leftValue;
  })[0];
}

function selectLatestAlphaHealthMetric(
  metrics: AlphaHealthMetricEvidence[],
): AlphaHealthMetricEvidence | undefined {
  return [...metrics].sort(
    (left, right) => Date.parse(right.recordedAtUtc) - Date.parse(left.recordedAtUtc),
  )[0];
}

function deriveLifecycleState(
  promotionDecisions: PromotionDecisionEvidence[],
  shadowEvaluations: ShadowEvaluationEvidence[],
): GovernanceLifecycleState {
  const allowedDecisions = [...promotionDecisions]
    .filter((decision) => toCanonicalState(decision.decisionState) === "allowed")
    .sort(
      (left, right) =>
        Date.parse(right.decidedAtUtc) - Date.parse(left.decidedAtUtc),
    );

  const latestAllowedDecision = allowedDecisions[0];
  if (!latestAllowedDecision) {
    const latestShadow = selectLatestShadowEvaluation(shadowEvaluations);
    if (
      latestShadow &&
      toCanonicalState(latestShadow.evaluationState) === "completed"
    ) {
      return "shadow";
    }
    return "draft";
  }

  const latestAction = toCanonicalState(latestAllowedDecision.lifecycleAction);
  if (latestAction === "retire") {
    return "deallocated";
  }

  const hasTwoLatestPromotes =
    allowedDecisions.length >= 2 &&
    toCanonicalState(allowedDecisions[0].lifecycleAction) === "promote" &&
    toCanonicalState(allowedDecisions[1].lifecycleAction) === "promote";
  if (hasTwoLatestPromotes) {
    return "production";
  }

  if (latestAction === "promote" || latestAction === "pause") {
    return "candidate-live";
  }

  const latestShadow = selectLatestShadowEvaluation(shadowEvaluations);
  if (latestShadow && toCanonicalState(latestShadow.evaluationState) === "completed") {
    return "shadow";
  }
  return "draft";
}

function recommendNextAction(missingArtifacts: string[]): string {
  if (missingArtifacts.length === 0) {
    return "Readiness signals are complete. Proceed through governed promotion/deallocation decision workflow.";
  }
  const hasValidationStageGap = missingArtifacts.some((item) =>
    item.startsWith("validation_stage:"),
  );
  const hasPromotionPacketGap = missingArtifacts.some((item) =>
    item.startsWith("promotion_packet_field:"),
  );
  const hasWindowGap = missingArtifacts.some((item) =>
    item.startsWith("alpha_health_window:"),
  );
  const hasShadowGap = missingArtifacts.includes("shadow_stability_evidence");

  if (hasValidationStageGap) {
    return "Run the missing FR7 validation stages and publish run-detail artifacts before making governance decisions.";
  }
  if (hasPromotionPacketGap) {
    return "Complete FR45 promotion packet fields (`data_quality_report`, `purged_cpcv_results`, `calibration_report`, `counterfactual_replay_summary`) before promotion review.";
  }
  if (hasWindowGap) {
    return "Publish FR10 alpha-health windows (`1h`, `24h`, `30d`) and re-evaluate guardrail readiness.";
  }
  if (hasShadowGap) {
    return "Complete shadow evaluation outputs so stability evidence is auditable before lifecycle progression.";
  }
  return "Resolve listed missing artifacts and re-run governance readiness evaluation.";
}

export function deriveAlphaGovernanceReadiness(
  input: ReadinessDerivationInput,
): AlphaGovernanceReadinessResult {
  const latestDecision = selectLatestDecision(input.promotionDecisions);
  const latestShadow = selectLatestShadowEvaluation(input.shadowEvaluations);
  const latestValidationRun = selectLatestValidationRun(input.validationRuns);
  const latestMetric = selectLatestAlphaHealthMetric(input.alphaHealthMetrics);

  const stageSet = new Set(
    input.validationArtifacts.map((artifact) => toCanonicalState(artifact.stage)),
  );
  const missingValidationStages = REQUIRED_VALIDATION_STAGES.filter(
    (stage) => !stageSet.has(stage),
  ).map((stage) => `validation_stage:${stage}`);

  const packetFieldSet = new Set<string>();
  if (latestDecision) {
    for (const key of Object.keys(latestDecision.evidencePacket)) {
      packetFieldSet.add(toCanonicalState(key));
    }
    for (const field of latestDecision.missingEvidenceFields) {
      packetFieldSet.delete(toCanonicalState(field));
    }
  }
  const missingPacketFields = REQUIRED_PROMOTION_PACKET_FIELDS.filter(
    (field) => !packetFieldSet.has(field),
  ).map((field) => `promotion_packet_field:${field}`);

  const shadowMissingEvidence =
    !latestShadow ||
    toCanonicalState(latestShadow.evaluationState) !== "completed" ||
    latestShadow.simulationOutcomeCount === 0
      ? ["shadow_stability_evidence"]
      : [];

  const windows = REQUIRED_FR10_WINDOWS.map((windowLabel) => {
    const sourceWindow = latestMetric?.windows.find(
      (entry) => toCanonicalState(entry.window) === windowLabel,
    );
    return {
      window: windowLabel,
      available: Boolean(sourceWindow),
      netPnl: sourceWindow?.netPnl,
      rollingSharpe: sourceWindow?.rollingSharpe,
      rollingHitRate: sourceWindow?.rollingHitRate,
      rollingDrawdown: sourceWindow?.rollingDrawdown,
      stabilityScore: sourceWindow?.stabilityScore,
      boundarySemantics: WINDOW_BOUNDARY_SEMANTICS,
    } satisfies GovernanceReadinessWindow;
  });
  const missingWindows = windows
    .filter((windowEvidence) => !windowEvidence.available)
    .map((windowEvidence) => `alpha_health_window:${windowEvidence.window}`);

  const missingArtifacts = [
    ...new Set([
      ...missingValidationStages,
      ...missingPacketFields,
      ...shadowMissingEvidence,
      ...missingWindows,
    ]),
  ];

  const lifecycleState = deriveLifecycleState(
    input.promotionDecisions,
    input.shadowEvaluations,
  );
  const validationCompleteness: GovernanceValidationCompleteness =
    missingValidationStages.length === 0 && missingPacketFields.length === 0
      ? "complete"
      : "incomplete";
  const shadowStability: GovernanceShadowStability =
    shadowMissingEvidence.length > 0
      ? !latestShadow
        ? "missing"
        : "unstable"
      : "stable";
  const guardrailState: GovernanceGuardrailState = latestMetric
    ? missingWindows.length > 0
      ? "missing"
      : input.alphaThresholdBreaches.length > 0
        ? "breached"
        : "healthy"
    : "missing";
  const readinessStatus: GovernanceReadinessStatus =
    missingArtifacts.length > 0 ? "blocked" : "ready";

  const asOfUtc = latestIsoTimestamp([
    latestDecision?.decidedAtUtc,
    latestShadow?.completedAtUtc ?? latestShadow?.startedAtUtc,
    latestValidationRun?.completedAtUtc ?? latestValidationRun?.startedAtUtc,
    latestMetric?.recordedAtUtc,
    ...input.metadata.map((entry) => entry.timestampUtc),
  ]);
  const latestMeta = [...input.metadata].sort(
    (left, right) => Date.parse(right.timestampUtc) - Date.parse(left.timestampUtc),
  )[0];
  const source = latestMeta?.endpoint ?? "/control/research";
  const correlationId =
    latestMeta?.correlationId ??
    latestDecision?.correlationId ??
    latestMetric?.correlationId ??
    "n/a";
  const reasonCode =
    readinessStatus === "ready"
      ? "governance_readiness_ready"
      : "governance_readiness_blocked";
  const guardrailReasonCodes = [
    ...new Set([
      ...(latestMetric ? [latestMetric.reasonCode] : []),
      ...input.alphaThresholdBreaches.map((breach) => breach.reasonCode),
      ...input.alphaThresholdBreaches.map((breach) => breach.breachReason),
    ]),
  ];

  return {
    candidateId: input.candidateId,
    alphaId: input.alphaId,
    lifecycleState,
    validationCompleteness,
    shadowStability,
    guardrailState,
    readinessStatus,
    missingArtifacts,
    recommendedNextAction: recommendNextAction(missingArtifacts),
    asOfUtc,
    source,
    reasonCode,
    correlationId,
    validationRunId: latestValidationRun?.runId,
    guardrailReasonCodes,
    guardrailWindows: windows,
  };
}

export async function queryAlphaGovernanceReadiness(
  input: QueryAlphaGovernanceReadinessInput,
): Promise<AlphaGovernanceReadinessResult> {
  const baseUrl = stripTrailingSlash(input.baseUrl.trim());
  if (!baseUrl) {
    throw new GovernanceReadinessClientError({
      status: 500,
      errorCode: "governance_readiness_base_url_missing",
      message:
        "Operator console API base URL is missing. Set NEXT_PUBLIC_OPERATOR_CONSOLE_API_BASE_URL.",
      action: "governance_readiness_query",
      endpoint: "/control/research",
      timestampUtc: new Date().toISOString(),
    });
  }

  const candidateId = normalizeCanonicalIdentifier("candidate_id", input.candidateId);
  const alphaId = normalizeCanonicalIdentifier("alpha_id", input.alphaId);
  const limit = resolveLimit(input.limit);
  const fetchImpl = resolveFetch(input.fetchImpl);

  const promotionEndpoint = buildEndpoint("/control/research/promotion-decisions", {
    candidate_id: candidateId,
    limit,
  });
  const shadowEndpoint = buildEndpoint("/control/research/shadow-evaluations", {
    candidate_id: candidateId,
    limit,
  });
  const validationRunsEndpoint = buildEndpoint("/control/research/validation-runs", {
    candidate_id: candidateId,
    limit,
  });
  const alphaHealthEndpoint = buildEndpoint("/control/research/alpha-health-metrics", {
    alpha_id: alphaId,
    limit,
  });
  const alphaBreachesEndpoint = buildEndpoint(
    "/control/research/alpha-threshold-breaches",
    {
      alpha_id: alphaId,
      limit,
    },
  );

  try {
    const [
      promotionsResponse,
      shadowResponse,
      validationRunsResponse,
      alphaHealthResponse,
      alphaBreachesResponse,
    ] = await Promise.all([
      requestEnvelope({
        baseUrl,
        endpoint: promotionEndpoint,
        fetchImpl,
        parseData: parsePromotionDecisionsData,
      }),
      requestEnvelope({
        baseUrl,
        endpoint: shadowEndpoint,
        fetchImpl,
        parseData: parseShadowEvaluationsData,
      }),
      requestEnvelope({
        baseUrl,
        endpoint: validationRunsEndpoint,
        fetchImpl,
        parseData: parseValidationRunsData,
      }),
      requestEnvelope({
        baseUrl,
        endpoint: alphaHealthEndpoint,
        fetchImpl,
        parseData: parseAlphaHealthMetricsData,
      }),
      requestEnvelope({
        baseUrl,
        endpoint: alphaBreachesEndpoint,
        fetchImpl,
        parseData: parseAlphaThresholdBreachesData,
      }),
    ]);

    const latestRun = selectLatestValidationRun(validationRunsResponse.data);
    let validationArtifacts: ValidationArtifactEvidence[] = [];
    const metadata = [
      promotionsResponse.meta,
      shadowResponse.meta,
      validationRunsResponse.meta,
      alphaHealthResponse.meta,
      alphaBreachesResponse.meta,
    ];
    if (latestRun) {
      const validationRunDetailEndpoint = `/control/research/validation-runs/${encodeURIComponent(
        latestRun.runId,
      )}`;
      const validationRunDetailResponse = await requestEnvelope({
        baseUrl,
        endpoint: validationRunDetailEndpoint,
        fetchImpl,
        parseData: parseValidationRunDetailData,
      });
      validationArtifacts = validationRunDetailResponse.data.artifacts;
      metadata.push(validationRunDetailResponse.meta);
    }

    return deriveAlphaGovernanceReadiness({
      candidateId,
      alphaId,
      promotionDecisions: promotionsResponse.data,
      shadowEvaluations: shadowResponse.data,
      validationRuns: validationRunsResponse.data,
      validationArtifacts,
      alphaHealthMetrics: alphaHealthResponse.data,
      alphaThresholdBreaches: alphaBreachesResponse.data,
      metadata,
    });
  } catch (error) {
    if (error instanceof GovernanceReadinessClientError) {
      throw error;
    }
    throw new GovernanceReadinessClientError({
      status: 500,
      errorCode: "governance_readiness_request_failed",
      message:
        error instanceof Error
          ? error.message
          : "Governance readiness query failed before receiving a response.",
      action: "governance_readiness_query",
      endpoint: "/control/research",
      timestampUtc: new Date().toISOString(),
    });
  }
}
