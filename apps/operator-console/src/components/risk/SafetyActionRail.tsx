"use client";

import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import {
  resolveConfirmationTabLoop,
  shouldDismissDangerConfirmation,
  type ConfirmationDialogControl,
} from "@/lib/risk/confirmation-dialog";
import {
  EmergencyControlClientError,
  getEmergencyControlActionResult,
  invokeRecoveryReadinessEvaluation,
  invokeRecoveryResume,
  queryRestoreRehearsalByRunId,
  queryRestoreRehearsals,
  invokeEmergencyControlAction,
  normalizeEmergencyAction,
  type EmergencyControlAction,
  type EmergencyControlDecision,
  type RecoveryReadinessDecision,
  type RecoveryRehearsalDecision,
  type RecoveryResumeDecision,
} from "@/lib/risk/control-actions";

type ActionButtonState =
  | "enabled"
  | "gated"
  | "in-progress"
  | "completed"
  | "blocked-with-reasons";

interface ResumeFailureDetail {
  gate: string;
  reasonCode: string;
  trigger: string;
  context: string;
  action: string;
  verification: string;
}

interface SafetyActionRailProps {
  baseUrl: string;
  resumeState: "gated" | "in-progress" | "completed" | "blocked-with-reasons";
  resumeStateReason: string;
  resumeFailureDetails: ResumeFailureDetail[];
  resumeProfileKey: string;
  resumeReconciliationRunId: string;
  resumeApprovedChecksum: string;
  resumeSignoffIntent: string;
  resumeArtifactId?: string;
  resumeIncidentCorrelationId?: string;
  resumeIncidentSeverity?: string;
  resumeRehearsalRunId?: string;
  onActionConfirmed: (decision: EmergencyControlDecision) => void;
  onRecoveryEvaluated: (decision: RecoveryReadinessDecision) => void;
  onRecoveryResumed: (decision: RecoveryResumeDecision) => void;
}

interface ActionTimingEvidence {
  acknowledgementMs: number;
  reflectionMs: number;
  confirmationMs: number;
}

interface ActionTimingWarning {
  code: "emergency_control_latency_threshold_exceeded";
  message: string;
  breaches: string[];
}

const ACTION_LABELS: Record<EmergencyControlAction, string> = {
  pause: "Pause",
  "reduce-only": "Reduce-only",
  "cancel-all": "Cancel-all",
};

const ACTION_HIERARCHY: Record<EmergencyControlAction, "danger" | "secondary"> = {
  pause: "danger",
  "reduce-only": "secondary",
  "cancel-all": "danger",
};

const DEFAULT_RESUME_STATE_REASON =
  "Resume remains gated until controlled recovery readiness evaluation succeeds.";

const ACKNOWLEDGEMENT_TARGET_MS = 1_000;
const REFLECTION_TARGET_MS = 5_000;
const CONFIRMATION_TARGET_MS = 10_000;
const REFLECTION_POLL_INTERVAL_MS = 300;
const RETRYABLE_RESULT_STATUS_CODES = new Set([404, 409, 425, 429, 500, 502, 503, 504]);
const RETRYABLE_RESULT_ERROR_CODES = new Set([
  "emergency_control_request_failed",
  "emergency_control_unknown_error",
  "emergency_control_response_parse_failed",
  "emergency_control_action_pending",
  "emergency_control_action_not_ready",
  "emergency_control_action_not_found",
]);

function toClientError(
  error: unknown,
  action: EmergencyControlAction | "resume",
): EmergencyControlClientError {
  if (error instanceof EmergencyControlClientError) {
    return error;
  }

  return new EmergencyControlClientError({
    status: 500,
    errorCode: "emergency_control_request_failed",
    message:
      error instanceof Error
        ? error.message
        : "Emergency control request failed unexpectedly.",
    action: action === "resume" ? "recovery_resume_execute" : action,
    endpoint: "operator-console-ui",
    timestampUtc: new Date().toISOString(),
  });
}

function isDangerAction(action: EmergencyControlAction): boolean {
  return action === "pause" || action === "cancel-all";
}

function isSevereIncidentSeverity(value: string | undefined): boolean {
  const normalized = value?.trim().toLowerCase();
  return normalized === "severity_1" || normalized === "severity_2";
}

function sleep(delayMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, delayMs);
  });
}

function isRetryableActionResultError(error: EmergencyControlClientError): boolean {
  return (
    RETRYABLE_RESULT_STATUS_CODES.has(error.status) ||
    RETRYABLE_RESULT_ERROR_CODES.has(error.errorCode)
  );
}

function resolveTimingWarning({
  action,
  acknowledgementMs,
  reflectionMs,
  confirmationMs,
}: {
  action: EmergencyControlAction;
  acknowledgementMs: number;
  reflectionMs: number;
  confirmationMs: number;
}): ActionTimingWarning | null {
  const breaches: string[] = [];

  if (acknowledgementMs > ACKNOWLEDGEMENT_TARGET_MS) {
    breaches.push(`acknowledgment ${acknowledgementMs}ms > ${ACKNOWLEDGEMENT_TARGET_MS}ms`);
  }
  if (reflectionMs > REFLECTION_TARGET_MS) {
    breaches.push(`reflected state ${reflectionMs}ms > ${REFLECTION_TARGET_MS}ms`);
  }
  if (confirmationMs > CONFIRMATION_TARGET_MS) {
    breaches.push(`confirmation ${confirmationMs}ms > ${CONFIRMATION_TARGET_MS}ms`);
  }

  if (breaches.length === 0) {
    return null;
  }

  return {
    code: "emergency_control_latency_threshold_exceeded",
    message: `${ACTION_LABELS[action]} succeeded but exceeded one or more latency thresholds.`,
    breaches,
  };
}

async function waitForConfirmedActionResult({
  baseUrl,
  actionId,
  action,
  startedAtMs,
}: {
  baseUrl: string;
  actionId: string;
  action: EmergencyControlAction;
  startedAtMs: number;
}): Promise<EmergencyControlDecision> {
  const deadlineMs = startedAtMs + CONFIRMATION_TARGET_MS;
  let lastRetryableError: EmergencyControlClientError | null = null;

  while (Date.now() <= deadlineMs) {
    try {
      return await getEmergencyControlActionResult({
        baseUrl,
        actionId,
      });
    } catch (error) {
      const clientError = toClientError(error, action);
      if (!isRetryableActionResultError(clientError)) {
        throw clientError;
      }
      lastRetryableError = clientError;
    }

    const remainingMs = deadlineMs - Date.now();
    if (remainingMs <= 0) {
      break;
    }
    await sleep(Math.min(REFLECTION_POLL_INTERVAL_MS, remainingMs));
  }

  throw new EmergencyControlClientError({
    status: lastRetryableError?.status ?? 504,
    errorCode: "emergency_control_confirmation_timeout",
    message:
      "Control confirmation could not be resolved within the <= 10s expectation window.",
    action,
    correlationId: lastRetryableError?.correlationId,
    endpoint: lastRetryableError?.endpoint ?? "operator-console-ui",
    timestampUtc: new Date().toISOString(),
  });
}

export function SafetyActionRail({
  baseUrl,
  resumeState,
  resumeStateReason,
  resumeFailureDetails,
  resumeProfileKey,
  resumeReconciliationRunId,
  resumeApprovedChecksum,
  resumeSignoffIntent,
  resumeArtifactId,
  resumeIncidentCorrelationId,
  resumeIncidentSeverity,
  resumeRehearsalRunId,
  onActionConfirmed,
  onRecoveryEvaluated,
  onRecoveryResumed,
}: SafetyActionRailProps) {
  const actionInFlightRef = useRef(false);
  const railRegionRef = useRef<HTMLElement | null>(null);
  const confirmationTriggerRef = useRef<EmergencyControlAction | "resume" | null>(
    null,
  );
  const actionButtonRefs = useRef<
    Record<EmergencyControlAction | "resume", HTMLButtonElement | null>
  >({
    pause: null,
    "reduce-only": null,
    "cancel-all": null,
    resume: null,
  });
  const confirmationConfirmButtonRef = useRef<HTMLButtonElement | null>(null);
  const confirmationCancelButtonRef = useRef<HTMLButtonElement | null>(null);
  const [pendingDangerAction, setPendingDangerAction] =
    useState<EmergencyControlAction | null>(null);
  const [activeAction, setActiveAction] = useState<
    EmergencyControlAction | "resume" | null
  >(
    null,
  );
  const [lastDecision, setLastDecision] = useState<EmergencyControlDecision | null>(
    null,
  );
  const [lastRecoveryReadiness, setLastRecoveryReadiness] =
    useState<RecoveryReadinessDecision | null>(null);
  const [lastRecoveryResume, setLastRecoveryResume] =
    useState<RecoveryResumeDecision | null>(null);
  const [lastRestoreRehearsal, setLastRestoreRehearsal] =
    useState<RecoveryRehearsalDecision | null>(null);
  const [timingEvidence, setTimingEvidence] = useState<ActionTimingEvidence | null>(
    null,
  );
  const [timingWarning, setTimingWarning] = useState<ActionTimingWarning | null>(null);
  const [error, setError] = useState<EmergencyControlClientError | null>(null);

  const completedAction = useMemo(() => {
    if (!lastDecision) {
      return undefined;
    }
    return normalizeEmergencyAction(lastDecision.action);
  }, [lastDecision]);

  const isBusy = activeAction !== null;
  const confirmationTitleId = pendingDangerAction
    ? `safety-action-confirmation-title-${pendingDangerAction}`
    : undefined;
  const confirmationDescriptionId = pendingDangerAction
    ? `safety-action-confirmation-description-${pendingDangerAction}`
    : undefined;

  const restoreFocusAfterConfirmation = ({
    target,
    trigger,
  }: {
    target: "trigger" | "rail";
    trigger?: EmergencyControlAction | "resume" | null;
  }) => {
    setTimeout(() => {
      if (target === "trigger") {
        const resolvedTrigger = trigger ?? confirmationTriggerRef.current;
        if (resolvedTrigger) {
          const button = actionButtonRefs.current[resolvedTrigger];
          if (button && !button.disabled) {
            button.focus();
            return;
          }
        }
      }
      railRegionRef.current?.focus();
    }, 0);
  };

  useEffect(() => {
    if (!pendingDangerAction) {
      return;
    }

    const handleWindowEscape = (event: KeyboardEvent) => {
      if (!shouldDismissDangerConfirmation({ key: event.key, isBusy })) {
        return;
      }
      event.preventDefault();
      const triggerForRestore = confirmationTriggerRef.current;
      setPendingDangerAction(null);
      restoreFocusAfterConfirmation({ target: "trigger", trigger: triggerForRestore });
    };

    window.addEventListener("keydown", handleWindowEscape);
    return () => {
      window.removeEventListener("keydown", handleWindowEscape);
    };
  }, [isBusy, pendingDangerAction]);

  const handleConfirmationTabLoop = (event: ReactKeyboardEvent<HTMLElement>) => {
    const activeControl: ConfirmationDialogControl | null =
      document.activeElement === confirmationConfirmButtonRef.current
        ? "confirm"
        : document.activeElement === confirmationCancelButtonRef.current
          ? "cancel"
          : null;
    const targetControl = resolveConfirmationTabLoop({
      key: event.key,
      shiftKey: event.shiftKey,
      activeControl,
    });
    if (!targetControl) {
      return;
    }
    event.preventDefault();
    if (targetControl === "confirm") {
      confirmationConfirmButtonRef.current?.focus();
      return;
    }
    confirmationCancelButtonRef.current?.focus();
  };

  const resolveState = (action: EmergencyControlAction | "resume"): ActionButtonState => {
    if (action === "resume") {
      if (activeAction === "resume") {
        return "in-progress";
      }
      if (isBusy) {
        return "gated";
      }
      if (lastRecoveryResume || resumeState === "completed") {
        return "completed";
      }
      if (
        isSevereIncidentSeverity(resumeIncidentSeverity) &&
        lastRestoreRehearsal?.rehearsalStatus === "failed"
      ) {
        return "blocked-with-reasons";
      }
      if (
        lastRecoveryReadiness?.readinessStatus === "blocked" ||
        resumeState === "blocked-with-reasons"
      ) {
        return "blocked-with-reasons";
      }
      return "enabled";
    }
    if (activeAction === action) {
      return "in-progress";
    }
    if (isBusy && activeAction !== action) {
      return "gated";
    }
    if (completedAction === action) {
      return "completed";
    }
    return "enabled";
  };

  const rehearsalFailureDetails: ResumeFailureDetail[] =
    lastRestoreRehearsal?.integrityChecks
      .filter((check) => !check.passed)
      .map((check) => ({
        gate: check.checkName,
        reasonCode: check.reasonCode,
        trigger: "Restore rehearsal integrity check",
        context: check.details,
        action: lastRestoreRehearsal.recommendedNextAction,
        verification:
          check.observedValue && check.expectedValue
            ? `expected=${check.expectedValue} observed=${check.observedValue}`
            : check.details,
      })) ?? [];

  const effectiveResumeReason =
    lastRecoveryReadiness?.recommendedNextAction ||
    (isSevereIncidentSeverity(resumeIncidentSeverity)
      ? lastRestoreRehearsal?.recommendedNextAction
      : undefined) ||
    resumeStateReason ||
    DEFAULT_RESUME_STATE_REASON;
  const effectiveResumeFailures: ResumeFailureDetail[] =
    lastRecoveryReadiness?.readinessStatus === "blocked"
      ? lastRecoveryReadiness.gateOutcomes
          .filter((outcome) => !outcome.passed)
          .map((outcome) => ({
            gate: outcome.gate,
            reasonCode: outcome.reasonCode,
            trigger: outcome.trigger,
            context: outcome.context,
            action: outcome.action,
            verification: outcome.verification,
          }))
      : isSevereIncidentSeverity(resumeIncidentSeverity) &&
          rehearsalFailureDetails.length > 0
        ? rehearsalFailureDetails
        : resumeFailureDetails;

  const executeAction = async (action: EmergencyControlAction) => {
    if (actionInFlightRef.current) {
      return;
    }

    const launchedFromConfirmation = pendingDangerAction !== null;
    actionInFlightRef.current = true;
    setPendingDangerAction(null);
    if (launchedFromConfirmation) {
      restoreFocusAfterConfirmation({ target: "rail" });
    }
    setError(null);
    setTimingEvidence(null);
    setTimingWarning(null);
    setActiveAction(action);

    const commandStart = Date.now();
    const auditReference = `operator-console-${action}-${commandStart}`;

    try {
      const accepted = await invokeEmergencyControlAction({
        baseUrl,
        action,
        auditReference,
      });
      const acknowledgementMs = Date.now() - commandStart;

      const confirmed = await waitForConfirmedActionResult({
        baseUrl,
        actionId: accepted.actionId,
        action,
        startedAtMs: commandStart,
      });
      const confirmationMs = Date.now() - commandStart;
      const reflectionMs = confirmationMs;
      const warning = resolveTimingWarning({
        action,
        acknowledgementMs,
        reflectionMs,
        confirmationMs,
      });

      setLastDecision(confirmed);
      setTimingEvidence({
        acknowledgementMs,
        reflectionMs,
        confirmationMs,
      });
      setTimingWarning(warning);
      onActionConfirmed(confirmed);
    } catch (caughtError) {
      setError(toClientError(caughtError, action));
    } finally {
      actionInFlightRef.current = false;
      setActiveAction(null);
    }
  };

  const executeResumeWorkflow = async () => {
    if (actionInFlightRef.current) {
      return;
    }

    actionInFlightRef.current = true;
    setPendingDangerAction(null);
    setError(null);
    setTimingEvidence(null);
    setTimingWarning(null);
    setActiveAction("resume");
    setLastRestoreRehearsal(null);

    const commandStart = Date.now();
    const auditReference = `operator-console-recovery-${commandStart}`;

    try {
      const severeIncident = isSevereIncidentSeverity(resumeIncidentSeverity);
      let rehearsalEvidence: RecoveryRehearsalDecision | null = null;
      if (resumeRehearsalRunId?.trim()) {
        rehearsalEvidence = await queryRestoreRehearsalByRunId({
          baseUrl,
          runId: resumeRehearsalRunId,
        });
      } else if (resumeIncidentCorrelationId?.trim() || resumeArtifactId?.trim()) {
        const rehearsalQuery = await queryRestoreRehearsals({
          baseUrl,
          correlationId: resumeIncidentCorrelationId,
          artifactId: resumeIncidentCorrelationId ? undefined : resumeArtifactId,
          limit: 1,
        });
        const latestRun = rehearsalQuery.rehearsals[0];
        if (latestRun) {
          rehearsalEvidence = await queryRestoreRehearsalByRunId({
            baseUrl,
            runId: latestRun.runId,
          });
        }
      }
      if (rehearsalEvidence) {
        setLastRestoreRehearsal(rehearsalEvidence);
      }
      if (
        severeIncident &&
        !rehearsalEvidence &&
        !resumeRehearsalRunId &&
        !resumeIncidentCorrelationId &&
        !resumeArtifactId
      ) {
        throw new EmergencyControlClientError({
          status: 400,
          errorCode: "recovery_rehearsal_selector_required",
          message:
            "Severity-1/Severity-2 resume requires rehearsal selector metadata (incident correlation, artifact, or rehearsal run).",
          action: "recovery_resume_execute",
          endpoint: "/control/recovery/resume",
          timestampUtc: new Date().toISOString(),
        });
      }
      if (severeIncident && rehearsalEvidence?.rehearsalStatus === "failed") {
        const acknowledgementMs = Date.now() - commandStart;
        setTimingEvidence({
          acknowledgementMs,
          reflectionMs: acknowledgementMs,
          confirmationMs: acknowledgementMs,
        });
        return;
      }

      const readiness = await invokeRecoveryReadinessEvaluation({
        baseUrl,
        profileKey: resumeProfileKey,
        reconciliationRunId: resumeReconciliationRunId,
        approvedChecksum: resumeApprovedChecksum,
        signoffIntent: resumeSignoffIntent,
        auditReference,
      });
      const acknowledgementMs = Date.now() - commandStart;
      setLastRecoveryReadiness(readiness);
      onRecoveryEvaluated(readiness);

      if (readiness.readinessStatus !== "approved") {
        setTimingEvidence({
          acknowledgementMs,
          reflectionMs: acknowledgementMs,
          confirmationMs: acknowledgementMs,
        });
        return;
      }

      const resumed = await invokeRecoveryResume({
        baseUrl,
        runId: readiness.runId,
        resumedAtUtc: new Date().toISOString(),
        incidentCorrelationId:
          resumeIncidentCorrelationId ??
          rehearsalEvidence?.incidentCorrelationId,
        artifactId: resumeArtifactId ?? rehearsalEvidence?.artifactId,
        incidentSeverity: resumeIncidentSeverity,
        rehearsalRunId: resumeRehearsalRunId ?? rehearsalEvidence?.runId,
      });
      const confirmationMs = Date.now() - commandStart;
      setLastRecoveryResume(resumed);
      setTimingEvidence({
        acknowledgementMs,
        reflectionMs: confirmationMs,
        confirmationMs,
      });
      onRecoveryResumed(resumed);
    } catch (caughtError) {
      setError(toClientError(caughtError, "resume"));
    } finally {
      actionInFlightRef.current = false;
      setActiveAction(null);
    }
  };

  return (
    <section
      aria-label="Persistent safety action rail"
      className="safety-action-rail"
      role="region"
      ref={railRegionRef}
      tabIndex={-1}
    >
      <div className="shell-panel-header">
        <div>
          <p className="type-eyebrow">Safety action rail</p>
          <h2 className="type-heading-m">Persistent emergency controls</h2>
        </div>
        <span
          className="shell-status-pill"
          data-tone={isBusy ? "warning" : "normal"}
        >
          {isBusy ? "in-progress" : "ready"}
        </span>
      </div>

      <p className="type-body text-muted">
        Urgent interventions stay within {"<= 2 interactions"} while dangerous
        controls enforce explicit confirmations. Recovery pathways preserve
        Story 3.7 controlled-recovery gate semantics.
      </p>
      <p className="type-metadata text-muted">
        Acknowledgment target {"<= 1s"} · reflected state target {"<= 5s"} ·
        timestamped confirmation target {"<= 10s"}
      </p>

      <ul className="safety-action-grid">
        {(Object.keys(ACTION_LABELS) as EmergencyControlAction[]).map((action) => {
          const state = resolveState(action);
          return (
            <li key={action}>
              <button
                aria-disabled={state === "gated" || state === "in-progress"}
                aria-label={`${ACTION_LABELS[action]} emergency control (${state} state)`}
                className="safety-action-button"
                data-hierarchy={ACTION_HIERARCHY[action]}
                data-state={state}
                disabled={state === "gated" || state === "in-progress"}
                ref={(node) => {
                  actionButtonRefs.current[action] = node;
                }}
                onClick={() => {
                  if (isDangerAction(action)) {
                    confirmationTriggerRef.current = action;
                    setPendingDangerAction(action);
                    return;
                  }
                  void executeAction(action);
                }}
                type="button"
              >
                {ACTION_LABELS[action]}
              </button>
              <p className="type-metadata text-muted">State: {state}</p>
            </li>
          );
        })}

        <li>
          {(() => {
            const resumeButtonState = resolveState("resume");
            const resumeButtonDisabled =
              resumeButtonState === "gated" || resumeButtonState === "in-progress";
            return (
              <>
                <button
                  aria-disabled={resumeButtonDisabled}
                  className="safety-action-button"
                  data-hierarchy="tertiary"
                  data-state={resumeButtonState}
                  disabled={resumeButtonDisabled}
                  ref={(node) => {
                    actionButtonRefs.current.resume = node;
                  }}
                  onClick={() => {
                    if (resumeButtonDisabled) {
                      return;
                    }
                    void executeResumeWorkflow();
                  }}
                  type="button"
                >
                  Resume
                </button>
                <p className="type-metadata text-muted">
                  State: {resumeButtonState}
                </p>
                <p className="type-metadata text-muted">
                  {effectiveResumeReason}
                </p>
              </>
            );
          })()}
        </li>
      </ul>

      {pendingDangerAction ? (
        <section
          aria-describedby={confirmationDescriptionId}
          aria-labelledby={confirmationTitleId}
          aria-live="assertive"
          aria-modal="true"
          className="safety-action-confirmation"
          onKeyDown={handleConfirmationTabLoop}
          role="alertdialog"
        >
          <h3 className="type-heading-m" id={confirmationTitleId}>
            Confirm {ACTION_LABELS[pendingDangerAction].toLowerCase()} action
          </h3>
          <p className="type-body" id={confirmationDescriptionId}>
            This dangerous action requires explicit operator intent confirmation.
          </p>
          <div className="safety-action-confirmation-controls">
            <button
              aria-disabled={isBusy}
              autoFocus
              className="safety-action-button"
              data-hierarchy="danger"
              data-state="enabled"
              disabled={isBusy}
              ref={confirmationConfirmButtonRef}
              onClick={() => {
                if (isBusy) {
                  return;
                }
                void executeAction(pendingDangerAction);
              }}
              type="button"
            >
              Confirm action
            </button>
            <button
              aria-disabled={isBusy}
              className="safety-action-button"
              data-hierarchy="secondary"
              data-state="enabled"
              disabled={isBusy}
              ref={confirmationCancelButtonRef}
              onClick={() => {
                const triggerForRestore = confirmationTriggerRef.current;
                setPendingDangerAction(null);
                restoreFocusAfterConfirmation({
                  target: "trigger",
                  trigger: triggerForRestore,
                });
              }}
              type="button"
            >
              Cancel
            </button>
          </div>
        </section>
      ) : null}

      {lastDecision && timingEvidence ? (
        <section className="safety-action-evidence" role="status">
          <p className="type-eyebrow">Post-action confirmation</p>
          <p className="assistive-announcement" aria-atomic="true" aria-live="assertive">
            Outcome: {lastDecision.resultingMode}. Reason: {lastDecision.reasonCode}.
            Timestamp: {lastDecision.timestampUtc}. Action ID: {lastDecision.actionId}.
            Correlation ID: {lastDecision.correlationId}.
          </p>
          <dl className="risk-evidence-grid">
            <div>
              <dt className="type-metadata text-muted">Action ID</dt>
              <dd className="type-mono">{lastDecision.actionId}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Actor/source</dt>
              <dd className="type-mono">
                {lastDecision.source} / {lastDecision.triggerSource}
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Resulting mode</dt>
              <dd className="type-mono">{lastDecision.resultingMode}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Reason code</dt>
              <dd className="type-mono">
                <code>{lastDecision.reasonCode}</code>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Correlation ID</dt>
              <dd className="type-mono">{lastDecision.correlationId}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Audit reference</dt>
              <dd className="type-mono">{lastDecision.auditReference ?? "n/a"}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Acknowledgment</dt>
              <dd className="type-mono">{timingEvidence.acknowledgementMs}ms</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Reflection</dt>
              <dd className="type-mono">{timingEvidence.reflectionMs}ms</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Confirmation</dt>
              <dd className="type-mono">{timingEvidence.confirmationMs}ms</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Timestamp</dt>
              <dd className="type-mono">
                <time dateTime={lastDecision.timestampUtc}>
                  {lastDecision.timestampUtc}
                </time>
              </dd>
            </div>
          </dl>
        </section>
      ) : null}

      {(resolveState("resume") === "blocked-with-reasons" &&
        effectiveResumeFailures.length > 0) ? (
        <section className="safety-action-recovery-blocked" role="status">
          <p className="type-eyebrow">Recovery resume blocked</p>
          <p className="type-body text-muted">{effectiveResumeReason}</p>
          <div className="safety-action-recovery-failures">
            {effectiveResumeFailures.map((failure) => (
              <article
                className="safety-action-recovery-failure"
                key={`${failure.gate}:${failure.reasonCode}`}
              >
                <h3 className="type-heading-m">{failure.gate}</h3>
                <p className="type-metadata text-muted">
                  <code>{failure.reasonCode}</code>
                </p>
                <ol className="type-metadata text-muted">
                  <li>
                    <strong>Trigger:</strong> {failure.trigger}
                  </li>
                  <li>
                    <strong>Context:</strong> {failure.context}
                  </li>
                  <li>
                    <strong>Action:</strong> {failure.action}
                  </li>
                  <li>
                    <strong>Verification:</strong> {failure.verification}
                  </li>
                </ol>
              </article>
            ))}
          </div>
        </section>
      ) : null}

      {lastRestoreRehearsal ? (
        <section className="safety-action-recovery-evidence" role="status">
          <p className="type-eyebrow">Restore rehearsal evidence</p>
          <p className="type-body text-muted">
            {lastRestoreRehearsal.recommendedNextAction}
          </p>
          <dl className="risk-evidence-grid">
            <div>
              <dt className="type-metadata text-muted">Run ID</dt>
              <dd className="type-mono">{lastRestoreRehearsal.runId}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Rehearsal status</dt>
              <dd className="type-mono">{lastRestoreRehearsal.rehearsalStatus}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Reason code</dt>
              <dd className="type-mono">
                <code>{lastRestoreRehearsal.reasonCode}</code>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Artifact ID</dt>
              <dd className="type-mono">{lastRestoreRehearsal.artifactId}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Deterministic replay</dt>
              <dd className="type-mono">
                {lastRestoreRehearsal.deterministicSignature.deterministicMatch === false
                  ? "mismatch"
                  : lastRestoreRehearsal.deterministicSignature.deterministicMatch === true
                    ? "match"
                    : "baseline"}
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Deterministic signature</dt>
              <dd className="type-mono">
                {lastRestoreRehearsal.deterministicSignature.deterministicSignature}
              </dd>
            </div>
          </dl>
          {lastRestoreRehearsal.integrityChecks.some((check) => !check.passed) ? (
            <div className="safety-action-recovery-failures">
              {lastRestoreRehearsal.integrityChecks
                .filter((check) => !check.passed)
                .map((check) => (
                  <article
                    className="safety-action-recovery-failure"
                    key={`${check.checkName}:${check.reasonCode}`}
                  >
                    <h3 className="type-heading-m">{check.checkName}</h3>
                    <p className="type-metadata text-muted">
                      <code>{check.reasonCode}</code>
                    </p>
                    <p className="type-metadata text-muted">{check.details}</p>
                  </article>
                ))}
            </div>
          ) : null}
        </section>
      ) : null}

      {lastRecoveryResume ? (
        <section className="safety-action-recovery-evidence" role="status">
          <p className="type-eyebrow">Controlled recovery verification</p>
          <dl className="risk-evidence-grid">
            <div>
              <dt className="type-metadata text-muted">Run ID</dt>
              <dd className="type-mono">{lastRecoveryResume.runId}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Readiness status</dt>
              <dd className="type-mono">{lastRecoveryResume.readinessStatus}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Reason code</dt>
              <dd className="type-mono">
                <code>{lastRecoveryResume.reasonCode}</code>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Verification reason</dt>
              <dd className="type-mono">
                <code>{lastRecoveryResume.verificationReasonCode}</code>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Resumed at</dt>
              <dd className="type-mono">
                <time dateTime={lastRecoveryResume.resumedAtUtc}>
                  {lastRecoveryResume.resumedAtUtc}
                </time>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Verification timestamp</dt>
              <dd className="type-mono">
                <time dateTime={lastRecoveryResume.verificationTimestampUtc}>
                  {lastRecoveryResume.verificationTimestampUtc}
                </time>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Correlation ID</dt>
              <dd className="type-mono">{lastRecoveryResume.correlationId}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Audit reference</dt>
              <dd className="type-mono">
                {lastRecoveryResume.auditReference ?? "n/a"}
              </dd>
            </div>
          </dl>
        </section>
      ) : null}

      {timingWarning ? (
        <section className="safety-action-warning" role="status">
          <p className="type-eyebrow">Timing threshold warning</p>
          <p className="type-body text-muted">{timingWarning.message}</p>
          <p className="type-metadata text-muted">
            <code>{timingWarning.code}</code>
          </p>
          <p className="type-metadata text-muted">
            {timingWarning.breaches.join(" · ")}
          </p>
        </section>
      ) : null}

      {error ? (
        <section className="safety-action-error" role="alert">
          <p className="type-eyebrow">Control failure</p>
          <p className="type-body text-muted">{error.message}</p>
          <dl className="risk-evidence-grid">
            <div>
              <dt className="type-metadata text-muted">Error code</dt>
              <dd className="type-mono">
                <code>{error.errorCode}</code>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Action</dt>
              <dd className="type-mono">{error.action}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Correlation ID</dt>
              <dd className="type-mono">{error.correlationId ?? "n/a"}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Timestamp</dt>
              <dd className="type-mono">
                <time dateTime={error.timestampUtc}>{error.timestampUtc}</time>
              </dd>
            </div>
          </dl>
        </section>
      ) : null}
    </section>
  );
}
