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
  resumeState: "gated";
  resumeStateReason: string;
  evidence: RiskPostureEvidence;
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
  "Resume is gated until Story 3.7 introduces controlled recovery readiness gates.";

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
      "Required action: keep locked-safe mode active until Story 3.7 recovery gates are available.",
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
    resumeState: "gated",
    resumeStateReason: RESUME_GATED_REASON,
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
