"use client";

import { useEffect, useMemo, useState } from "react";
import {
  IncidentAlertClientError,
  queryIncidentAlerts,
  type IncidentAlertsQueryResult,
} from "@/lib/incidents/alerts";
import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

interface IncidentAlertsPanelProps {
  freshness: ShellFreshnessSnapshot;
  baseUrl: string;
}

type ViewState = "loading" | "ready" | "empty" | "error" | "critical";

function toClientError(error: unknown): IncidentAlertClientError {
  if (error instanceof IncidentAlertClientError) {
    return error;
  }
  return new IncidentAlertClientError({
    status: 500,
    errorCode: "incident_alert_request_failed",
    reasonCode: "incident_alert_request_failed",
    message:
      error instanceof Error
        ? error.message
        : "Incident alert request failed unexpectedly.",
    action: "incident_alerts_query",
    endpoint: "operator-console-ui",
    timestampUtc: new Date().toISOString(),
  });
}

function isCriticalFailure(error: IncidentAlertClientError): boolean {
  return (
    error.status >= 500 ||
    error.errorCode.includes("dependency_unavailable") ||
    error.errorCode.includes("stale_evidence")
  );
}

function statusTone(state: ViewState): "normal" | "warning" | "critical" {
  if (state === "critical") {
    return "critical";
  }
  if (state === "loading" || state === "empty" || state === "error") {
    return "warning";
  }
  return "normal";
}

function statusLabel(state: ViewState): string {
  if (state === "loading") {
    return "loading";
  }
  if (state === "empty") {
    return "empty";
  }
  if (state === "error") {
    return "error";
  }
  if (state === "critical") {
    return "critical";
  }
  return "ready";
}

function viewRecommendedAction(
  result: IncidentAlertsQueryResult | null,
  state: ViewState,
): string {
  if (result) {
    return result.recommendedNextAction;
  }
  if (state === "critical") {
    return "Execute containment controls and verify runbook-linked evidence before proceeding.";
  }
  if (state === "error") {
    return "Retry incident alert query and confirm dependency-health telemetry.";
  }
  return "Monitor warning and critical alert states to keep incident response deterministic.";
}

export function IncidentAlertsPanel({ freshness, baseUrl }: IncidentAlertsPanelProps) {
  const [state, setState] = useState<ViewState>("loading");
  const [result, setResult] = useState<IncidentAlertsQueryResult | null>(null);
  const [requestError, setRequestError] = useState<IncidentAlertClientError | null>(null);
  const [refreshTick, setRefreshTick] = useState(0);
  const [lastAnnouncementAtUtc, setLastAnnouncementAtUtc] = useState(
    freshness.lastUpdatedIso,
  );

  useEffect(() => {
    let cancelled = false;

    async function run() {
      try {
        const next = await queryIncidentAlerts({
          baseUrl,
          limit: 25,
        });
        if (cancelled) {
          return;
        }
        setRequestError(null);
        setResult(next);
        setLastAnnouncementAtUtc(next.timestampUtc);
        if (next.dataState === "empty") {
          setState("empty");
          return;
        }
        const hasCritical = next.alerts.some((item) => item.severity === "critical");
        setState(hasCritical ? "critical" : "ready");
      } catch (error) {
        if (cancelled) {
          return;
        }
        const clientError = toClientError(error);
        setRequestError(clientError);
        setResult(null);
        setState(isCriticalFailure(clientError) ? "critical" : "error");
        setLastAnnouncementAtUtc(clientError.timestampUtc);
      }
    }

    void run();
    return () => {
      cancelled = true;
    };
  }, [baseUrl, refreshTick]);

  const metadata = useMemo(() => {
    if (result) {
      return {
        source: result.source,
        reasonCode: result.reasonCode,
        correlationId: result.correlationId,
      };
    }
    return {
      source: freshness.source,
      reasonCode:
        state === "critical"
          ? requestError?.reasonCode ?? "alert_dependency_unavailable"
          : state === "error"
            ? requestError?.reasonCode ?? "alert_invalid_payload"
            : "alert_loading",
      correlationId: requestError?.correlationId ?? "n/a",
    };
  }, [freshness.source, requestError, result, state]);

  return (
    <article
      aria-busy={state === "loading"}
      aria-label="Severity alerts with recommended operator actions"
      className="shell-panel shell-panel-grid-item incident-alerts-panel"
    >
      <div className="shell-panel-header">
        <p className="type-eyebrow">Severity alert delivery</p>
        <span className="shell-status-pill" data-tone={statusTone(state)}>
          {statusLabel(state)}
        </span>
      </div>

      <h2 className="type-heading-m">Warning and critical operator alerts</h2>
      <p className="type-body text-muted">
        Alerts include impacted subsystem, cause, and one clear recommended next action with
        runbook evidence links.
      </p>

      <dl className="incident-alert-metadata-grid">
        <div>
          <dt className="type-metadata text-muted">Source</dt>
          <dd className="type-mono">{metadata.source}</dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Reason code</dt>
          <dd className="type-mono">
            <code>{metadata.reasonCode}</code>
          </dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Correlation ID</dt>
          <dd className="type-mono">{metadata.correlationId}</dd>
        </div>
      </dl>

      <p className="type-metadata incident-alert-view-action" data-tone={statusTone(state)}>
        Recommended next action: {viewRecommendedAction(result, state)}
      </p>

      <section
        className="incident-announcement-surface"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        <p className="type-metadata">
          Status <code>{statusLabel(state)}</code> at{" "}
          <time dateTime={lastAnnouncementAtUtc}>{lastAnnouncementAtUtc}</time>.{" "}
          {result ? (
            <>
              Reason <code>{result.reasonCode}</code>. Correlation{" "}
              <code>{result.correlationId}</code>.
            </>
          ) : requestError ? (
            <>
              Reason <code>{requestError.reasonCode}</code>. Correlation{" "}
              <code>{requestError.correlationId ?? "n/a"}</code>.
            </>
          ) : (
            <>Awaiting incident alert response.</>
          )}
        </p>
      </section>

      <button
        className="allocation-form-submit allocation-form-submit--secondary"
        type="button"
        onClick={() => {
          setState("loading");
          setResult(null);
          setRequestError(null);
          setLastAnnouncementAtUtc(new Date().toISOString());
          setRefreshTick((value) => value + 1);
        }}
      >
        Refresh alerts
      </button>

      {state === "loading" ? (
        <section className="incident-alert-skeleton" role="status" aria-live="polite">
          <p className="type-eyebrow">Loading severity alert evidence</p>
          <div className="incident-alert-skeleton-line" />
          <div className="incident-alert-skeleton-line incident-alert-skeleton-line--short" />
          <div className="incident-alert-skeleton-line" />
        </section>
      ) : null}

      {state === "empty" ? (
        <section className="incident-alert-empty-state" role="status">
          <p className="type-eyebrow">No warning or critical alerts</p>
          <p className="type-body text-muted">
            Alert pipeline is healthy for the current evidence window. Keep monitoring trigger
            conditions and readiness telemetry.
          </p>
        </section>
      ) : null}

      {requestError && (state === "error" || state === "critical") ? (
        <section className="incident-alert-error-surface" role="alert">
          <p className="type-eyebrow">Incident alert query failure</p>
          <p className="type-body text-muted">{requestError.message}</p>
          <p className="type-metadata">
            Error code: <code>{requestError.errorCode}</code>
          </p>
          <p className="type-metadata">
            Action: {requestError.action} · correlation ID:{" "}
            {requestError.correlationId ?? "n/a"}
          </p>
        </section>
      ) : null}

      {result && result.alerts.length > 0 ? (
        <ol className="incident-alert-list">
          {result.alerts.map((alertItem) => (
            <li
              key={alertItem.alertId}
              className="incident-alert-item"
              data-severity={alertItem.severity}
            >
              <div className="incident-alert-item-header">
                <span className="type-eyebrow">{alertItem.severity}</span>
                <time className="type-mono" dateTime={alertItem.issuedAt}>
                  {alertItem.issuedAt}
                </time>
              </div>

              <p className="type-body">
                <strong>Impacted subsystem:</strong> {alertItem.impactedSubsystem}
              </p>
              <p className="type-body">
                <strong>Cause:</strong> {alertItem.cause}
              </p>
              <p className="type-metadata incident-alert-next-action">
                Recommended next action: {alertItem.recommendedNextAction}
              </p>
              <p className="type-metadata text-muted">
                Status <code>{alertItem.status}</code> · reason <code>{alertItem.reasonCode}</code>{" "}
                · correlation <code>{alertItem.correlationId}</code>
              </p>
              <p className="type-metadata">
                <a
                  className="incident-alert-runbook-link"
                  href={alertItem.evidenceLink}
                  rel="noreferrer"
                  target="_blank"
                >
                  Runbook evidence
                </a>
              </p>

              <ol className="incident-alert-attempt-list">
                {alertItem.attempts.map((attempt) => (
                  <li key={`${alertItem.alertId}-${attempt.attemptNumber}`}>
                    <p className="type-metadata">
                      Attempt {attempt.attemptNumber} · channel <code>{attempt.channel}</code> ·
                      outcome <code>{attempt.outcome}</code> · reason{" "}
                      <code>{attempt.reasonCode}</code>
                    </p>
                    <p className="type-metadata text-muted">
                      attempted {attempt.attemptedAt}
                      {attempt.deliveredAt ? ` · delivered ${attempt.deliveredAt}` : ""}
                      {attempt.failedAt ? ` · failed ${attempt.failedAt}` : ""}
                    </p>
                  </li>
                ))}
              </ol>
            </li>
          ))}
        </ol>
      ) : null}
    </article>
  );
}
