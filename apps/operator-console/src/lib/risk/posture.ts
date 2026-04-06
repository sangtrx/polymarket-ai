import type {
  RecoveryGateOutcomeEvidence,
  RecoveryReadinessDecision,
  RecoveryResumeDecision,
} from "./control-actions";

export type ShellSearchParams = Record<string, string | string[] | undefined>;

export interface ShellReadModelSnapshotLike {
  dataState: "ready" | "loading" | "empty" | "error" | "unauthorized";
  freshness: {
    lastUpdatedIso: string;
    source: string;
    isStale: boolean;
  };
  p95TargetMs: number;
}

export type RiskPosture = "normal" | "warning" | "critical" | "locked-safe";

export interface RiskPostureEvidence {
  lastUpdatedIso: string;
  source: string;
  resultingMode: string;
  reasonCode: string;
  actorSource: string;
  actionId?: string;
  correlationId?: string;
  auditReference?: string;
}

export interface RiskPostureViewModel {
  posture: RiskPosture;
  headline: string;
  summary: string;
  recommendedNextAction: string;
  requiredActionGuidance: string;
  ariaLive: "polite" | "assertive";
  interactionBudget: 2;
  acknowledgementTargetMs: 1_000;
  reflectionTargetMs: 5_000;
  confirmationTargetMs: 10_000;
  resumeState: "gated" | "in-progress" | "completed" | "blocked-with-reasons";
  resumeStateReason: string;
  resumeFailureDetails: RecoveryGateFailureDetail[];
  resumeRunId?: string;
  resumeVerifiedAtIso?: string;
  resumeProfileKey: string;
  resumeReconciliationRunId: string;
  resumeApprovedChecksum: string;
  resumeSignoffIntent: string;
  evidence: RiskPostureEvidence;
}

export interface RecoveryGateFailureDetail {
  gate: string;
  reasonCode: string;
  trigger: string;
  context: string;
  action: string;
  verification: string;
}

export interface EmergencyControlEvidenceInput {
  actionId: string;
  source: string;
  triggerSource: string;
  resultingMode: string;
  reasonCode: string;
  correlationId: string;
  timestampUtc: string;
  auditReference?: string;
}

const RESUME_GATED_REASON =
  "Run controlled recovery readiness evaluation before resuming normal trading.";
const DEFAULT_RESUME_PROFILE_KEY = "default";
const DEFAULT_RESUME_RECONCILIATION_RUN_ID = "recon-default";
const DEFAULT_RESUME_APPROVED_CHECKSUM =
  "0000000000000000000000000000000000000000000000000000000000000000";
const DEFAULT_RESUME_SIGNOFF_INTENT =
  "Operator confirms controlled recovery readiness evidence.";

const SOURCE_PATTERN = /^[a-z0-9._:-]+$/i;

const POSTURE_DETAILS: Record<
  RiskPosture,
  {
    headline: string;
    summary: string;
    recommendedNextAction: string;
    requiredActionGuidance: string;
    fallbackMode: string;
    fallbackReasonCode: string;
  }
> = {
  normal: {
    headline: "Risk posture normal",
    summary:
      "Portfolio telemetry is within configured limits and intervention controls are ready.",
    recommendedNextAction:
      "Continue monitoring and keep emergency controls ready for immediate use.",
    requiredActionGuidance:
      "No immediate intervention required while freshness and exposure signals remain healthy.",
    fallbackMode: "normal",
    fallbackReasonCode: "risk_posture_normal",
  },
  warning: {
    headline: "Risk posture warning",
    summary:
      "Conditions are elevated. Reduced-risk intervention may be required if drift worsens.",
    recommendedNextAction:
      "Prepare reduce-only mode and confirm read-model freshness before placing additional risk.",
    requiredActionGuidance:
      "One clear next action: use reduce-only if exposure drift continues or freshness degrades.",
    fallbackMode: "reduce_only",
    fallbackReasonCode: "risk_posture_warning",
  },
  critical: {
    headline: "Risk posture critical",
    summary:
      "Immediate intervention is required to prevent unsafe trading side effects.",
    recommendedNextAction:
      "Trigger pause immediately and validate resulting control evidence.",
    requiredActionGuidance:
      "Required action: pause trading through the safety rail and verify confirmation metadata.",
    fallbackMode: "paused",
    fallbackReasonCode: "risk_posture_critical",
  },
  "locked-safe": {
    headline: "Risk posture locked-safe",
    summary:
      "System is in a protective locked-safe state. Resume remains disabled until readiness gates pass.",
    recommendedNextAction:
      "Maintain containment and follow governance recovery checklist.",
    requiredActionGuidance:
      "Required action: run controlled recovery readiness evaluation before executing resume.",
    fallbackMode: "paused",
    fallbackReasonCode: "risk_posture_locked_safe",
  },
};

function firstValue(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) {
    return value[0];
  }
  return value;
}

function normalizeOptionalToken(
  value: string | undefined,
  maxLength = 180,
): string | undefined {
  const normalized = value?.trim();
  if (!normalized) {
    return undefined;
  }
  if (normalized.length > maxLength) {
    return undefined;
  }
  return normalized;
}

function normalizeSource(value: string | undefined, fallback: string): string {
  const normalized = value?.trim();
  if (
    normalized &&
    normalized.length <= 120 &&
    SOURCE_PATTERN.test(normalized)
  ) {
    return normalized;
  }
  return fallback;
}

function normalizeTimestamp(raw: string | undefined, fallback: string): string {
  if (raw) {
    const parsed = Date.parse(raw);
    if (!Number.isNaN(parsed)) {
      return new Date(parsed).toISOString();
    }
  }

  const fallbackParsed = Date.parse(fallback);
  if (!Number.isNaN(fallbackParsed)) {
    return new Date(fallbackParsed).toISOString();
  }
  return new Date().toISOString();
}

function normalizeOptionalTimestamp(raw: string | undefined): string | undefined {
  if (!raw) {
    return undefined;
  }
  const parsed = Date.parse(raw);
  if (Number.isNaN(parsed)) {
    return undefined;
  }
  return new Date(parsed).toISOString();
}

function normalizePosture(value: string | undefined): RiskPosture | undefined {
  const normalized = value?.trim().toLowerCase();
  if (!normalized) {
    return undefined;
  }

  if (normalized === "locked_safe") {
    return "locked-safe";
  }

  if (
    normalized === "normal" ||
    normalized === "warning" ||
    normalized === "critical" ||
    normalized === "locked-safe"
  ) {
    return normalized;
  }

  return undefined;
}

function fallbackPosture(shellSnapshot: ShellReadModelSnapshotLike): RiskPosture {
  if (shellSnapshot.dataState === "unauthorized") {
    return "locked-safe";
  }
  if (shellSnapshot.dataState === "error") {
    return "critical";
  }
  if (shellSnapshot.freshness.isStale || shellSnapshot.dataState !== "ready") {
    return "warning";
  }
  return "normal";
}

function resolveAriaLive(posture: RiskPosture): "polite" | "assertive" {
  return posture === "critical" || posture === "locked-safe"
    ? "assertive"
    : "polite";
}

function normalizeResultingMode(value: string | undefined, posture: RiskPosture): string {
  const normalized = value?.trim().toLowerCase();
  if (normalized === "reduce-only") {
    return "reduce_only";
  }
  if (
    normalized === "normal" ||
    normalized === "paused" ||
    normalized === "reduce_only"
  ) {
    return normalized;
  }
  return POSTURE_DETAILS[posture].fallbackMode;
}

function normalizeReasonCode(value: string | undefined, posture: RiskPosture): string {
  return (
    normalizeOptionalToken(value, 180) ?? POSTURE_DETAILS[posture].fallbackReasonCode
  );
}

function normalizeResumeState(
  value: string | undefined,
): RiskPostureViewModel["resumeState"] | undefined {
  const normalized = value?.trim().toLowerCase();
  if (!normalized) {
    return undefined;
  }
  if (
    normalized === "gated" ||
    normalized === "in-progress" ||
    normalized === "completed" ||
    normalized === "blocked-with-reasons" ||
    normalized === "blocked_with_reasons"
  ) {
    return normalized === "blocked_with_reasons"
      ? "blocked-with-reasons"
      : (normalized as RiskPostureViewModel["resumeState"]);
  }
  return undefined;
}

function normalizeChecksum(value: string | undefined): string | undefined {
  const normalized = value?.trim().toLowerCase();
  if (!normalized || !/^[0-9a-f]{64}$/.test(normalized)) {
    return undefined;
  }
  return normalized;
}

export function deriveRiskPostureFromMode(mode: string): RiskPosture {
  const normalized = mode.trim().toLowerCase();
  if (normalized === "paused") {
    return "locked-safe";
  }
  if (normalized === "reduce_only" || normalized === "reduce-only") {
    return "warning";
  }
  if (normalized === "normal") {
    return "normal";
  }
  return "critical";
}

export function resolveRiskPostureViewModel(
  searchParams: ShellSearchParams,
  shellSnapshot: ShellReadModelSnapshotLike,
): RiskPostureViewModel {
  const explicitPosture = normalizePosture(
    firstValue(searchParams.riskState) ?? firstValue(searchParams.posture),
  );
  const posture = explicitPosture ?? fallbackPosture(shellSnapshot);
  const details = POSTURE_DETAILS[posture];
  const evidenceSource = normalizeSource(
    firstValue(searchParams.riskSource),
    shellSnapshot.freshness.source,
  );
  const lastUpdatedIso = normalizeTimestamp(
    firstValue(searchParams.at),
    shellSnapshot.freshness.lastUpdatedIso,
  );

  const resumeState =
    normalizeResumeState(firstValue(searchParams.resumeState)) ?? "gated";
  const resumeStateReason =
    normalizeOptionalToken(firstValue(searchParams.resumeStateReason), 240) ??
    RESUME_GATED_REASON;

  return {
    posture,
    headline: details.headline,
    summary: details.summary,
    recommendedNextAction: details.recommendedNextAction,
    requiredActionGuidance: details.requiredActionGuidance,
    ariaLive: resolveAriaLive(posture),
    interactionBudget: 2,
    acknowledgementTargetMs: 1_000,
    reflectionTargetMs: 5_000,
    confirmationTargetMs: 10_000,
    resumeState,
    resumeStateReason,
    resumeFailureDetails: [],
    resumeRunId: normalizeOptionalToken(firstValue(searchParams.resumeRunId), 180),
    resumeVerifiedAtIso: normalizeOptionalTimestamp(
      firstValue(searchParams.resumeVerifiedAtUtc),
    ),
    resumeProfileKey:
      normalizeOptionalToken(firstValue(searchParams.resumeProfileKey), 120) ??
      DEFAULT_RESUME_PROFILE_KEY,
    resumeReconciliationRunId:
      normalizeOptionalToken(
        firstValue(searchParams.resumeReconciliationRunId),
        180,
      ) ?? DEFAULT_RESUME_RECONCILIATION_RUN_ID,
    resumeApprovedChecksum:
      normalizeChecksum(firstValue(searchParams.resumeApprovedChecksum)) ??
      DEFAULT_RESUME_APPROVED_CHECKSUM,
    resumeSignoffIntent:
      normalizeOptionalToken(firstValue(searchParams.resumeSignoffIntent), 200) ??
      DEFAULT_RESUME_SIGNOFF_INTENT,
    evidence: {
      lastUpdatedIso,
      source: evidenceSource,
      resultingMode: normalizeResultingMode(firstValue(searchParams.mode), posture),
      reasonCode: normalizeReasonCode(firstValue(searchParams.reasonCode), posture),
      actorSource:
        normalizeOptionalToken(firstValue(searchParams.actorSource), 120) ??
        "operator_console_ui",
      actionId: normalizeOptionalToken(firstValue(searchParams.actionId), 180),
      correlationId: normalizeOptionalToken(
        firstValue(searchParams.correlationId),
        180,
      ),
      auditReference: normalizeOptionalToken(
        firstValue(searchParams.auditReference),
        180,
      ),
    },
  };
}

export function withEmergencyControlEvidence(
  current: RiskPostureViewModel,
  evidence: EmergencyControlEvidenceInput,
): RiskPostureViewModel {
  const posture = deriveRiskPostureFromMode(evidence.resultingMode);
  const details = POSTURE_DETAILS[posture];
  return {
    ...current,
    posture,
    headline: details.headline,
    summary: details.summary,
    recommendedNextAction: details.recommendedNextAction,
    requiredActionGuidance: details.requiredActionGuidance,
    ariaLive: resolveAriaLive(posture),
    resumeState: "gated",
    resumeStateReason: RESUME_GATED_REASON,
    resumeFailureDetails: [],
    resumeRunId: undefined,
    resumeVerifiedAtIso: undefined,
    evidence: {
      ...current.evidence,
      lastUpdatedIso: normalizeTimestamp(
        evidence.timestampUtc,
        current.evidence.lastUpdatedIso,
      ),
      source: normalizeSource(evidence.source, current.evidence.source),
      resultingMode: normalizeResultingMode(evidence.resultingMode, posture),
      reasonCode: normalizeReasonCode(evidence.reasonCode, posture),
      actorSource:
        normalizeOptionalToken(evidence.triggerSource, 120) ??
        current.evidence.actorSource,
      actionId: normalizeOptionalToken(evidence.actionId, 180),
      correlationId:
        normalizeOptionalToken(evidence.correlationId, 180) ??
        current.evidence.correlationId,
      auditReference:
        normalizeOptionalToken(evidence.auditReference, 180) ??
        current.evidence.auditReference,
    },
  };
}

function toRecoveryGateFailureDetail(
  outcome: RecoveryGateOutcomeEvidence,
): RecoveryGateFailureDetail {
  return {
    gate: outcome.gate,
    reasonCode: outcome.reasonCode,
    trigger: outcome.trigger,
    context: outcome.context,
    action: outcome.action,
    verification: outcome.verification,
  };
}

export function withRecoveryReadinessEvidence(
  current: RiskPostureViewModel,
  decision: RecoveryReadinessDecision,
): RiskPostureViewModel {
  const blockedFailures = decision.gateOutcomes
    .filter((outcome) => !outcome.passed)
    .map(toRecoveryGateFailureDetail);
  const isApproved = decision.readinessStatus === "approved";
  const posture = isApproved ? "locked-safe" : "locked-safe";
  const details = POSTURE_DETAILS[posture];

  return {
    ...current,
    posture,
    headline: details.headline,
    summary: details.summary,
    recommendedNextAction: details.recommendedNextAction,
    requiredActionGuidance: details.requiredActionGuidance,
    ariaLive: resolveAriaLive(posture),
    resumeState: isApproved ? "in-progress" : "blocked-with-reasons",
    resumeStateReason: decision.recommendedNextAction,
    resumeFailureDetails: blockedFailures,
    resumeRunId: decision.runId,
    resumeVerifiedAtIso: undefined,
    resumeProfileKey: decision.profileKey,
    resumeReconciliationRunId:
      decision.reconciliationRunId ?? current.resumeReconciliationRunId,
    resumeApprovedChecksum:
      decision.approvedChecksum ?? current.resumeApprovedChecksum,
    evidence: {
      ...current.evidence,
      lastUpdatedIso: normalizeTimestamp(
        decision.evaluatedAtUtc,
        current.evidence.lastUpdatedIso,
      ),
      source: "control_api.recovery.readiness",
      reasonCode: decision.reasonCode,
      actionId: decision.runId,
      correlationId: decision.correlationId,
      auditReference: decision.auditReference ?? current.evidence.auditReference,
    },
  };
}

export function withRecoveryResumeEvidence(
  current: RiskPostureViewModel,
  decision: RecoveryResumeDecision,
): RiskPostureViewModel {
  const details = POSTURE_DETAILS.normal;
  return {
    ...current,
    posture: "normal",
    headline: details.headline,
    summary: details.summary,
    recommendedNextAction: details.recommendedNextAction,
    requiredActionGuidance: details.requiredActionGuidance,
    ariaLive: resolveAriaLive("normal"),
    resumeState: "completed",
    resumeStateReason:
      "Controlled recovery resume is verified. Continue monitoring post-resume telemetry.",
    resumeFailureDetails: [],
    resumeRunId: decision.runId,
    resumeVerifiedAtIso: normalizeTimestamp(
      decision.verificationTimestampUtc,
      current.evidence.lastUpdatedIso,
    ),
    evidence: {
      ...current.evidence,
      lastUpdatedIso: normalizeTimestamp(
        decision.verificationTimestampUtc,
        current.evidence.lastUpdatedIso,
      ),
      source: "control_api.recovery.resume",
      resultingMode: "normal",
      reasonCode: decision.verificationReasonCode,
      actionId: decision.runId,
      correlationId: decision.correlationId,
      auditReference: decision.auditReference ?? current.evidence.auditReference,
    },
  };
}
