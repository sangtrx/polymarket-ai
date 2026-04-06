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

export type RecoveryRehearsalStatus = "passed" | "failed";

export interface RecoveryRehearsalIntegrityCheck {
  checkName: string;
  passed: boolean;
  reasonCode: string;
  expectedValue?: string;
  observedValue?: string;
  details: string;
}

export interface RecoveryDeterministicSignatureEvidence {
  deterministicSignature: string;
  priorSignature?: string;
  deterministicMatch?: boolean;
  mismatchSummary?: string;
}

export interface RecoveryRehearsalDecision {
  status: "accepted";
  action: string;
  runId: string;
  rehearsalStatus: RecoveryRehearsalStatus;
  reasonCode: string;
  actorId: string;
  actorRole: string;
  correlationId: string;
  artifactId: string;
  artifactChecksum: string;
  observedChecksum: string;
  restoreTarget: string;
  reconciliationRunId: string;
  reconciliationMismatchRate?: number;
  reconciliationPassed: boolean;
  requestedAtUtc: string;
  startedAtUtc: string;
  completedAtUtc: string;
  integrityChecks: RecoveryRehearsalIntegrityCheck[];
  deterministicSignature: RecoveryDeterministicSignatureEvidence;
  recommendedNextAction: string;
  incidentCorrelationId?: string;
  incidentSeverity?: string;
  auditReference?: string;
  timestampUtc: string;
}

export interface RecoveryRehearsalRunSummary {
  runId: string;
  rehearsalStatus: RecoveryRehearsalStatus;
  reasonCode: string;
  artifactId: string;
  correlationId: string;
  completedAtUtc: string;
  failingChecks: string[];
  recommendedNextAction: string;
}

export interface RecoveryRehearsalQueryDecision {
  status: "accepted";
  action: string;
  actorId: string;
  role: string;
  correlationId: string;
  timestampUtc: string;
  rehearsals: RecoveryRehearsalRunSummary[];
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
  incidentCorrelationId?: string;
  artifactId?: string;
  incidentSeverity?: string;
  rehearsalRunId?: string;
  fetchImpl?: FetchLike;
}

interface RestoreRehearsalRequestInput {
  baseUrl: string;
  artifactId: string;
  artifactChecksum: string;
  restoreTarget: string;
  reconciliationRunId: string;
  restoreOutput: Record<string, unknown>;
  observedChecksum?: string;
  incidentCorrelationId?: string;
  incidentSeverity?: string;
  auditReference?: string;
  fetchImpl?: FetchLike;
}

interface RehearsalRunQueryInput {
  baseUrl: string;
  runId: string;
  fetchImpl?: FetchLike;
}

interface RehearsalQueryInput {
  baseUrl: string;
  artifactId?: string;
  correlationId?: string;
  limit?: number;
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
const RECOVERY_REHEARSALS_ENDPOINT = "/control/recovery/rehearsals";
const INCIDENT_SEVERITY_PATTERN = /^severity_[1-4]$/;

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

function requiredChecksumString(
  record: Record<string, unknown>,
  key: string,
  context: string,
): string {
  const resolved = requiredString(record, key, context);
  if (!CHECKSUM_PATTERN.test(resolved)) {
    throw new EmergencyControlClientError({
      status: 502,
      errorCode: "recovery_contract_mismatch",
      message: `expected ${key} to be lowercase 64-char hex digest in ${context}`,
      action: "recovery_contract_validation",
      endpoint: context,
      timestampUtc: new Date().toISOString(),
    });
  }
  return resolved;
}

function optionalChecksumString(
  record: Record<string, unknown>,
  key: string,
  context: string,
): string | undefined {
  const resolved = optionalString(record, key);
  if (typeof resolved === "undefined") {
    return undefined;
  }
  if (!CHECKSUM_PATTERN.test(resolved)) {
    throw new EmergencyControlClientError({
      status: 502,
      errorCode: "recovery_contract_mismatch",
      message: `expected ${key} to be lowercase 64-char hex digest when present in ${context}`,
      action: "recovery_contract_validation",
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

function optionalFiniteNumber(
  record: Record<string, unknown>,
  key: string,
): number | undefined {
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

function parseRecoveryRehearsalStatus(
  payload: Record<string, unknown>,
  endpoint: string,
): RecoveryRehearsalStatus {
  const rehearsalStatus = requiredString(payload, "rehearsal_status", endpoint);
  if (rehearsalStatus === "passed" || rehearsalStatus === "failed") {
    return rehearsalStatus;
  }
  throw new EmergencyControlClientError({
    status: 502,
    errorCode: "recovery_contract_mismatch",
    message: `Expected rehearsal_status to be passed|failed, received '${rehearsalStatus}'.`,
    action: "recovery_contract_validation",
    endpoint,
    timestampUtc: new Date().toISOString(),
  });
}

function parseRecoveryRehearsalIntegrityChecks(
  payload: Record<string, unknown>,
  endpoint: string,
): RecoveryRehearsalIntegrityCheck[] {
  const checksRaw = payload.integrity_checks;
  if (!Array.isArray(checksRaw)) {
    throw new EmergencyControlClientError({
      status: 502,
      errorCode: "recovery_contract_mismatch",
      message: "Expected integrity_checks array in restore rehearsal response.",
      action: "recovery_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  return checksRaw.map((candidate, index) => {
    if (!isRecord(candidate)) {
      throw new EmergencyControlClientError({
        status: 502,
        errorCode: "recovery_contract_mismatch",
        message: `Expected object integrity_checks[${index}]`,
        action: "recovery_contract_validation",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
    return {
      checkName: requiredString(candidate, "check_name", endpoint),
      passed: requiredBoolean(candidate, "passed", endpoint),
      reasonCode: requiredString(candidate, "reason_code", endpoint),
      expectedValue: optionalString(candidate, "expected_value"),
      observedValue: optionalString(candidate, "observed_value"),
      details: requiredString(candidate, "details", endpoint),
    };
  });
}

function parseRecoveryDeterministicSignature(
  payload: Record<string, unknown>,
  endpoint: string,
): RecoveryDeterministicSignatureEvidence {
  const candidate = payload.deterministic_signature;
  if (!isRecord(candidate)) {
    throw new EmergencyControlClientError({
      status: 502,
      errorCode: "recovery_contract_mismatch",
      message: "Expected deterministic_signature object in restore rehearsal response.",
      action: "recovery_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const matchCandidate = candidate.deterministic_match;
  return {
    deterministicSignature: requiredChecksumString(
      candidate,
      "deterministic_signature",
      endpoint,
    ),
    priorSignature: optionalChecksumString(candidate, "prior_signature", endpoint),
    deterministicMatch:
      typeof matchCandidate === "boolean" ? matchCandidate : undefined,
    mismatchSummary: optionalString(candidate, "mismatch_summary"),
  };
}

function parseRecoveryRehearsalPayload(
  payload: unknown,
  endpoint: string,
  status: number,
): RecoveryRehearsalDecision {
  if (!isRecord(payload)) {
    throw new EmergencyControlClientError({
      status,
      errorCode: "recovery_contract_mismatch",
      message: "Expected object payload for restore rehearsal response.",
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
    rehearsalStatus: parseRecoveryRehearsalStatus(payload, endpoint),
    reasonCode: requiredString(payload, "reason_code", endpoint),
    actorId: requiredString(payload, "actor_id", endpoint),
    actorRole: requiredString(payload, "actor_role", endpoint),
    correlationId: requiredString(payload, "correlation_id", endpoint),
    artifactId: requiredString(payload, "artifact_id", endpoint),
    artifactChecksum: requiredChecksumString(payload, "artifact_checksum", endpoint),
    observedChecksum: requiredChecksumString(payload, "observed_checksum", endpoint),
    restoreTarget: requiredString(payload, "restore_target", endpoint),
    reconciliationRunId: requiredString(payload, "reconciliation_run_id", endpoint),
    reconciliationMismatchRate: optionalFiniteNumber(
      payload,
      "reconciliation_mismatch_rate",
    ),
    reconciliationPassed: requiredBoolean(payload, "reconciliation_passed", endpoint),
    requestedAtUtc: normalizeTimestamp(
      requiredString(payload, "requested_at_utc", endpoint),
    ),
    startedAtUtc: normalizeTimestamp(
      requiredString(payload, "started_at_utc", endpoint),
    ),
    completedAtUtc: normalizeTimestamp(
      requiredString(payload, "completed_at_utc", endpoint),
    ),
    integrityChecks: parseRecoveryRehearsalIntegrityChecks(payload, endpoint),
    deterministicSignature: parseRecoveryDeterministicSignature(payload, endpoint),
    recommendedNextAction: requiredString(payload, "recommended_next_action", endpoint),
    incidentCorrelationId: optionalString(payload, "incident_correlation_id"),
    incidentSeverity: optionalString(payload, "incident_severity"),
    auditReference: optionalString(payload, "audit_reference"),
    timestampUtc: normalizeTimestamp(
      requiredString(payload, "timestamp_utc", endpoint),
    ),
  };
}

function parseRecoveryRehearsalQueryPayload(
  payload: unknown,
  endpoint: string,
  status: number,
): RecoveryRehearsalQueryDecision {
  if (!isRecord(payload)) {
    throw new EmergencyControlClientError({
      status,
      errorCode: "recovery_contract_mismatch",
      message: "Expected object payload for restore rehearsal query response.",
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
  const rehearsalsRaw = payload.rehearsals;
  if (!Array.isArray(rehearsalsRaw)) {
    throw new EmergencyControlClientError({
      status: 502,
      errorCode: "recovery_contract_mismatch",
      message: "Expected rehearsals array in restore rehearsal query response.",
      action: "recovery_contract_validation",
      endpoint,
      timestampUtc: new Date().toISOString(),
    });
  }
  const rehearsals = rehearsalsRaw.map((candidate, index) => {
    if (!isRecord(candidate)) {
      throw new EmergencyControlClientError({
        status: 502,
        errorCode: "recovery_contract_mismatch",
        message: `Expected object rehearsals[${index}]`,
        action: "recovery_contract_validation",
        endpoint,
        timestampUtc: new Date().toISOString(),
      });
    }
    const failingChecksRaw = candidate.failing_checks;
    return {
      runId: requiredString(candidate, "run_id", endpoint),
      rehearsalStatus: parseRecoveryRehearsalStatus(candidate, endpoint),
      reasonCode: requiredString(candidate, "reason_code", endpoint),
      artifactId: requiredString(candidate, "artifact_id", endpoint),
      correlationId: requiredString(candidate, "correlation_id", endpoint),
      completedAtUtc: normalizeTimestamp(
        requiredString(candidate, "completed_at_utc", endpoint),
      ),
      failingChecks: Array.isArray(failingChecksRaw)
        ? failingChecksRaw
            .filter((value): value is string => typeof value === "string")
            .map((value) => value.trim())
            .filter((value) => value.length > 0)
        : [],
      recommendedNextAction: requiredString(
        candidate,
        "recommended_next_action",
        endpoint,
      ),
    };
  });

  return {
    status: "accepted",
    action: requiredString(payload, "action", endpoint),
    actorId: requiredString(payload, "actor_id", endpoint),
    role: requiredString(payload, "role", endpoint),
    correlationId: requiredString(payload, "correlation_id", endpoint),
    timestampUtc: normalizeTimestamp(requiredString(payload, "timestamp_utc", endpoint)),
    rehearsals,
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
  incidentCorrelationId,
  artifactId,
  incidentSeverity,
  rehearsalRunId,
  fetchImpl,
}: RecoveryResumeRequestInput): Promise<RecoveryResumeDecision> {
  const runId = requestedRunId.trim();
  const normalizedIncidentCorrelationId = incidentCorrelationId?.trim();
  const normalizedArtifactId = artifactId?.trim();
  const normalizedIncidentSeverity = incidentSeverity?.trim().toLowerCase();
  const normalizedRehearsalRunId = rehearsalRunId?.trim();
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
  if (normalizedArtifactId && !ACTION_ID_PATTERN.test(normalizedArtifactId)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "recovery_invalid_artifact_id",
      message: "Recovery resume artifact_id selector must be a valid identifier.",
      action: "recovery_resume_execute",
      endpoint: RECOVERY_RESUME_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (
    normalizedIncidentCorrelationId &&
    !ACTION_ID_PATTERN.test(normalizedIncidentCorrelationId)
  ) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "recovery_invalid_incident_correlation_id",
      message: "Recovery resume incident_correlation_id selector must be valid.",
      action: "recovery_resume_execute",
      endpoint: RECOVERY_RESUME_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (
    normalizedIncidentSeverity &&
    !INCIDENT_SEVERITY_PATTERN.test(normalizedIncidentSeverity)
  ) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "recovery_invalid_incident_severity",
      message:
        "Recovery resume incident_severity must be one of severity_1..severity_4.",
      action: "recovery_resume_execute",
      endpoint: RECOVERY_RESUME_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (normalizedRehearsalRunId && !ACTION_ID_PATTERN.test(normalizedRehearsalRunId)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "recovery_invalid_rehearsal_run_id",
      message: "Recovery resume rehearsal_run_id selector must be valid.",
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
      incident_correlation_id: normalizedIncidentCorrelationId,
      artifact_id: normalizedArtifactId,
      incident_severity: normalizedIncidentSeverity,
      rehearsal_run_id: normalizedRehearsalRunId,
    }),
    fallbackAction: "recovery_resume_execute",
    fetchImpl,
    parseSuccess: parseRecoveryResumePayload,
  });
}

export async function invokeRestoreRehearsal({
  baseUrl,
  artifactId,
  artifactChecksum,
  restoreTarget,
  reconciliationRunId,
  restoreOutput,
  observedChecksum,
  incidentCorrelationId,
  incidentSeverity,
  auditReference,
  fetchImpl,
}: RestoreRehearsalRequestInput): Promise<RecoveryRehearsalDecision> {
  const normalizedArtifactId = artifactId.trim();
  const normalizedChecksum = artifactChecksum.trim().toLowerCase();
  const normalizedRestoreTarget = restoreTarget.trim();
  const normalizedReconciliationRunId = reconciliationRunId.trim();
  const normalizedObservedChecksum = observedChecksum?.trim().toLowerCase();
  const normalizedIncidentCorrelationId = incidentCorrelationId?.trim();
  const normalizedIncidentSeverity = incidentSeverity?.trim().toLowerCase();
  if (!normalizedArtifactId || !ACTION_ID_PATTERN.test(normalizedArtifactId)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_artifact_id",
      message: "Restore rehearsal requires a valid artifact_id.",
      action: "recovery_rehearsal_execute",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (!CHECKSUM_PATTERN.test(normalizedChecksum)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_checksum",
      message:
        "Restore rehearsal requires artifact_checksum as lowercase 64-char hex.",
      action: "recovery_rehearsal_execute",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (!normalizedRestoreTarget) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_restore_target",
      message: "Restore rehearsal requires a non-empty restore_target.",
      action: "recovery_rehearsal_execute",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (!normalizedReconciliationRunId) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_reconciliation_run_id",
      message: "Restore rehearsal requires a non-empty reconciliation_run_id.",
      action: "recovery_rehearsal_execute",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (
    normalizedObservedChecksum &&
    !CHECKSUM_PATTERN.test(normalizedObservedChecksum)
  ) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_observed_checksum",
      message:
        "Restore rehearsal observed_checksum must be lowercase 64-char hex when provided.",
      action: "recovery_rehearsal_execute",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (
    normalizedIncidentCorrelationId &&
    !ACTION_ID_PATTERN.test(normalizedIncidentCorrelationId)
  ) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_incident_correlation_id",
      message: "Restore rehearsal incident_correlation_id selector must be valid.",
      action: "recovery_rehearsal_execute",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (
    normalizedIncidentSeverity &&
    !INCIDENT_SEVERITY_PATTERN.test(normalizedIncidentSeverity)
  ) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_incident_severity",
      message:
        "Restore rehearsal incident_severity must be one of severity_1..severity_4.",
      action: "recovery_rehearsal_execute",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (!restoreOutput || typeof restoreOutput !== "object") {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_restore_output",
      message:
        "Restore rehearsal requires restore_output as a structured object payload.",
      action: "recovery_rehearsal_execute",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }

  return performRecoveryRequest({
    baseUrl,
    endpoint: RECOVERY_REHEARSALS_ENDPOINT,
    method: "POST",
    body: JSON.stringify({
      artifact_id: normalizedArtifactId,
      artifact_checksum: normalizedChecksum,
      restore_target: normalizedRestoreTarget,
      reconciliation_run_id: normalizedReconciliationRunId,
      restore_output: restoreOutput,
      observed_checksum: normalizedObservedChecksum,
      incident_correlation_id: normalizedIncidentCorrelationId,
      incident_severity: normalizedIncidentSeverity,
      audit_reference: auditReference,
    }),
    fallbackAction: "recovery_rehearsal_execute",
    fetchImpl,
    parseSuccess: parseRecoveryRehearsalPayload,
  });
}

export async function queryRestoreRehearsalByRunId({
  baseUrl,
  runId: requestedRunId,
  fetchImpl,
}: RehearsalRunQueryInput): Promise<RecoveryRehearsalDecision> {
  const runId = requestedRunId.trim();
  if (!runId || !ACTION_ID_PATTERN.test(runId)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_run_id",
      message: "Restore rehearsal query by run requires a valid run_id.",
      action: "recovery_rehearsal_query",
      endpoint: `${RECOVERY_REHEARSALS_ENDPOINT}/{run_id}`,
      timestampUtc: new Date().toISOString(),
    });
  }
  return performRecoveryRequest({
    baseUrl,
    endpoint: `${RECOVERY_REHEARSALS_ENDPOINT}/${encodeURIComponent(runId)}`,
    method: "GET",
    fallbackAction: "recovery_rehearsal_query",
    fetchImpl,
    parseSuccess: parseRecoveryRehearsalPayload,
  });
}

export async function queryRestoreRehearsals({
  baseUrl,
  artifactId,
  correlationId,
  limit,
  fetchImpl,
}: RehearsalQueryInput): Promise<RecoveryRehearsalQueryDecision> {
  const normalizedArtifactId = artifactId?.trim();
  const normalizedCorrelationId = correlationId?.trim();
  if (normalizedArtifactId && normalizedCorrelationId) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_query_ambiguous_selector",
      message:
        "Restore rehearsal query requires either artifactId or correlationId, not both.",
      action: "recovery_rehearsal_query",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (!normalizedArtifactId && !normalizedCorrelationId) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_query_missing_selector",
      message: "Restore rehearsal query requires artifactId or correlationId selector.",
      action: "recovery_rehearsal_query",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (normalizedArtifactId && !ACTION_ID_PATTERN.test(normalizedArtifactId)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_artifact_id",
      message: "Restore rehearsal artifactId selector must be valid.",
      action: "recovery_rehearsal_query",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  if (normalizedCorrelationId && !ACTION_ID_PATTERN.test(normalizedCorrelationId)) {
    throw new EmergencyControlClientError({
      status: 400,
      errorCode: "rehearsal_invalid_correlation_id",
      message: "Restore rehearsal correlationId selector must be valid.",
      action: "recovery_rehearsal_query",
      endpoint: RECOVERY_REHEARSALS_ENDPOINT,
      timestampUtc: new Date().toISOString(),
    });
  }
  const boundedLimit =
    typeof limit === "number" && Number.isFinite(limit)
      ? Math.max(1, Math.min(200, Math.trunc(limit)))
      : 20;
  const selector = normalizedArtifactId
    ? `artifact_id=${encodeURIComponent(normalizedArtifactId)}`
    : `correlation_id=${encodeURIComponent(normalizedCorrelationId ?? "")}`;
  return performRecoveryRequest({
    baseUrl,
    endpoint: `${RECOVERY_REHEARSALS_ENDPOINT}?${selector}&limit=${boundedLimit}`,
    method: "GET",
    fallbackAction: "recovery_rehearsal_query",
    fetchImpl,
    parseSuccess: parseRecoveryRehearsalQueryPayload,
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
