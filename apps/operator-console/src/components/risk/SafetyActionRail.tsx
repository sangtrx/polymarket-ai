"use client";

import { useMemo, useRef, useState } from "react";
import {
  EmergencyControlClientError,
  getEmergencyControlActionResult,
  invokeEmergencyControlAction,
  normalizeEmergencyAction,
  type EmergencyControlAction,
  type EmergencyControlDecision,
} from "@/lib/risk/control-actions";

type ActionButtonState = "enabled" | "gated" | "in-progress" | "completed";

interface SafetyActionRailProps {
  baseUrl: string;
  resumeState: "gated";
  resumeStateReason: string;
  onActionConfirmed: (decision: EmergencyControlDecision) => void;
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
  "Resume is gated until Story 3.7 introduces controlled recovery readiness gates.";

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
    action: action === "resume" ? "emergency_control_resume_gated" : action,
    endpoint: "operator-console-ui",
    timestampUtc: new Date().toISOString(),
  });
}

function isDangerAction(action: EmergencyControlAction): boolean {
  return action === "pause" || action === "cancel-all";
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
  onActionConfirmed,
}: SafetyActionRailProps) {
  const actionInFlightRef = useRef(false);
  const [pendingDangerAction, setPendingDangerAction] =
    useState<EmergencyControlAction | null>(null);
  const [activeAction, setActiveAction] = useState<EmergencyControlAction | null>(
    null,
  );
  const [lastDecision, setLastDecision] = useState<EmergencyControlDecision | null>(
    null,
  );
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

  const resolveState = (action: EmergencyControlAction | "resume"): ActionButtonState => {
    if (action === "resume") {
      return resumeState;
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

  const executeAction = async (action: EmergencyControlAction) => {
    if (actionInFlightRef.current) {
      return;
    }

    actionInFlightRef.current = true;
    setPendingDangerAction(null);
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

  return (
    <section
      aria-label="Persistent safety action rail"
      className="safety-action-rail"
      role="region"
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
        controls enforce explicit confirmations.
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
                className="safety-action-button"
                data-hierarchy={ACTION_HIERARCHY[action]}
                data-state={state}
                disabled={state === "gated" || state === "in-progress"}
                onClick={() => {
                  if (isDangerAction(action)) {
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
          <button
            aria-disabled="true"
            className="safety-action-button"
            data-hierarchy="tertiary"
            data-state={resolveState("resume")}
            disabled
            type="button"
          >
            Resume
          </button>
          <p className="type-metadata text-muted">
            {resumeStateReason || DEFAULT_RESUME_STATE_REASON}
          </p>
        </li>
      </ul>

      {pendingDangerAction ? (
        <section
          aria-describedby={confirmationDescriptionId}
          aria-labelledby={confirmationTitleId}
          aria-live="assertive"
          aria-modal="false"
          className="safety-action-confirmation"
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
              onClick={() => setPendingDangerAction(null)}
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
