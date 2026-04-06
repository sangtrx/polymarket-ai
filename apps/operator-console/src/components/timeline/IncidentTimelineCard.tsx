"use client";

import { useEffect, useMemo, useState } from "react";
import {
  IncidentForensicsClientError,
  queryIncidentForensics,
  type IncidentForensicsQueryResult,
} from "@/lib/incidents/forensics";
import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

interface IncidentTimelineCardProps {
  freshness: ShellFreshnessSnapshot;
  baseUrl: string;
}

type ViewState = "loading" | "ready" | "empty" | "error" | "critical";

interface IncidentSearchDraft {
  marketId: string;
  orderId: string;
  alphaId: string;
  actorId: string;
  startTs: string;
  endTs: string;
}

interface SubmittedIncidentFilters {
  marketId?: string;
  orderId?: string;
  alphaId?: string;
  actorId?: string;
  startTs: string;
  endTs: string;
}

function defaultWindow(): { startTs: string; endTs: string } {
  const end = new Date();
  const start = new Date(end.getTime() - 24 * 60 * 60 * 1000);
  return {
    startTs: start.toISOString(),
    endTs: end.toISOString(),
  };
}

function toDateTimeLocalValue(isoTimestamp: string): string {
  const parsed = new Date(isoTimestamp);
  if (Number.isNaN(parsed.getTime())) {
    return "";
  }
  const timezoneOffsetMs = parsed.getTimezoneOffset() * 60_000;
  return new Date(parsed.getTime() - timezoneOffsetMs).toISOString().slice(0, 16);
}

function fromDateTimeLocalValue(value: string): string {
  const normalized = value.trim();
  if (!normalized) {
    return "";
  }
  const parsed = new Date(normalized);
  if (Number.isNaN(parsed.getTime())) {
    return normalized;
  }
  return parsed.toISOString();
}

function toClientError(error: unknown): IncidentForensicsClientError {
  if (error instanceof IncidentForensicsClientError) {
    return error;
  }
  return new IncidentForensicsClientError({
    status: 500,
    errorCode: "incident_request_failed",
    reasonCode: "incident_request_failed",
    message:
      error instanceof Error
        ? error.message
        : "Incident forensics request failed unexpectedly.",
    action: "incident_query",
    endpoint: "operator-console-ui",
    timestampUtc: new Date().toISOString(),
  });
}

function isCriticalFailure(error: IncidentForensicsClientError): boolean {
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

function normalizeInput(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function recommendedAction(
  state: ViewState,
  result: IncidentForensicsQueryResult | null,
): string {
  if (result) {
    return result.recommendedNextAction;
  }
  if (state === "critical") {
    return "Trigger containment controls and verify correlated evidence before proceeding.";
  }
  if (state === "error") {
    return "Adjust filters and retry with canonical identifiers and UTC timestamps.";
  }
  return "Submit an incident query to build Trigger -> Context -> Action -> Verification flow.";
}

export function IncidentTimelineCard({ freshness, baseUrl }: IncidentTimelineCardProps) {
  const windowDefaults = useMemo(() => defaultWindow(), []);
  const [draft, setDraft] = useState<IncidentSearchDraft>({
    marketId: "",
    orderId: "",
    alphaId: "",
    actorId: "",
    startTs: toDateTimeLocalValue(windowDefaults.startTs),
    endTs: toDateTimeLocalValue(windowDefaults.endTs),
  });
  const [submitted, setSubmitted] = useState<SubmittedIncidentFilters>({
    startTs: windowDefaults.startTs,
    endTs: windowDefaults.endTs,
  });
  const [refreshTick, setRefreshTick] = useState(0);
  const [state, setState] = useState<ViewState>("loading");
  const [result, setResult] = useState<IncidentForensicsQueryResult | null>(null);
  const [requestError, setRequestError] = useState<IncidentForensicsClientError | null>(
    null,
  );

  useEffect(() => {
    let cancelled = false;

    async function run() {
      try {
        const next = await queryIncidentForensics({
          baseUrl,
          marketId: submitted.marketId,
          orderId: submitted.orderId,
          alphaId: submitted.alphaId,
          actorId: submitted.actorId,
          startTs: submitted.startTs,
          endTs: submitted.endTs,
        });
        if (cancelled) {
          return;
        }
        setRequestError(null);
        setResult(next);
        if (next.dataState === "empty") {
          setState("empty");
          return;
        }
        setState(
          next.severity === "critical" || next.severity === "degraded"
            ? "critical"
            : "ready",
        );
      } catch (error) {
        if (cancelled) {
          return;
        }
        const clientError = toClientError(error);
        setRequestError(clientError);
        setResult(null);
        setState(isCriticalFailure(clientError) ? "critical" : "error");
      }
    }

    void run();
    return () => {
      cancelled = true;
    };
  }, [baseUrl, submitted, refreshTick]);

  const metadata = useMemo(() => {
    if (result) {
      return {
        source: result.source,
        reasonCode: result.reasonCode,
        correlationId: result.correlationId,
        startInclusiveUtc: result.startInclusiveUtc,
        endExclusiveUtc: result.endExclusiveUtc,
        queryLatencyMs: result.queryLatencyMs,
        p95LatencyTargetMs: result.p95LatencyTargetMs,
      };
    }
    return {
      source: freshness.source,
      reasonCode:
        state === "critical"
          ? requestError?.reasonCode ?? "incident_dependency_unavailable"
          : state === "error"
            ? requestError?.reasonCode ?? "incident_invalid_payload"
            : "incident_loading",
      correlationId: requestError?.correlationId ?? "n/a",
      startInclusiveUtc: submitted.startTs,
      endExclusiveUtc: submitted.endTs,
      queryLatencyMs: 0,
      p95LatencyTargetMs: 5_000,
    };
  }, [freshness.source, requestError, result, state, submitted.endTs, submitted.startTs]);

  const filterSummary = useMemo(() => {
    const filters = result?.filters ?? {
      marketId: submitted.marketId,
      orderId: submitted.orderId,
      alphaId: submitted.alphaId,
      actorId: submitted.actorId,
    };
    const parts = [
      filters.marketId ? `market_id=${filters.marketId}` : null,
      filters.orderId ? `order_id=${filters.orderId}` : null,
      filters.alphaId ? `alpha_id=${filters.alphaId}` : null,
      filters.actorId ? `actor_id=${filters.actorId}` : null,
    ].filter((value): value is string => Boolean(value));
    return parts.length > 0 ? parts.join(" · ") : "none (window-only search)";
  }, [result, submitted.actorId, submitted.alphaId, submitted.marketId, submitted.orderId]);

  return (
    <article
      className="shell-panel shell-panel-grid-item incident-timeline-card"
      aria-busy={state === "loading"}
      aria-label="Incident search and causal timeline forensics"
    >
      <div className="shell-panel-header">
        <p className="type-eyebrow">Incident search + causal timeline</p>
        <span className="shell-status-pill" data-tone={statusTone(state)}>
          {statusLabel(state)}
        </span>
      </div>

      <h2 className="type-heading-m">Single-submit incident forensics</h2>
      <p className="type-body text-muted">
        Trigger -&gt; Context -&gt; Action -&gt; Verification remains visible with
        machine-readable evidence at every step.
      </p>

      <dl className="incident-metadata-grid">
        <div>
          <dt className="type-metadata text-muted">Window start (inclusive)</dt>
          <dd className="type-mono">{metadata.startInclusiveUtc}</dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Window end (exclusive)</dt>
          <dd className="type-mono">{metadata.endExclusiveUtc}</dd>
        </div>
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
        <div>
          <dt className="type-metadata text-muted">Latency</dt>
          <dd className="type-mono">
            {metadata.queryLatencyMs}ms / p95 target {metadata.p95LatencyTargetMs}ms
          </dd>
        </div>
      </dl>

      <p className="type-metadata text-muted">
        Effective filters: <code>{filterSummary}</code>
      </p>

      <p className="type-metadata incident-next-action" data-tone={statusTone(state)}>
        Recommended next action: {recommendedAction(state, result)}
      </p>

      <form
        className="incident-filter-grid"
        onSubmit={(event) => {
          event.preventDefault();
          setState("loading");
          setResult(null);
          setRequestError(null);
          setSubmitted({
            marketId: normalizeInput(draft.marketId),
            orderId: normalizeInput(draft.orderId),
            alphaId: normalizeInput(draft.alphaId),
            actorId: normalizeInput(draft.actorId),
            startTs: fromDateTimeLocalValue(draft.startTs),
            endTs: fromDateTimeLocalValue(draft.endTs),
          });
        }}
      >
        <label className="incident-filter-field">
          <span className="type-metadata">Market ID</span>
          <input
            className="incident-filter-input"
            value={draft.marketId}
            placeholder="market-btc-election"
            onChange={(event) =>
              setDraft((current) => ({ ...current, marketId: event.target.value }))
            }
          />
        </label>

        <label className="incident-filter-field">
          <span className="type-metadata">Order ID</span>
          <input
            className="incident-filter-input"
            value={draft.orderId}
            placeholder="order-101"
            onChange={(event) =>
              setDraft((current) => ({ ...current, orderId: event.target.value }))
            }
          />
        </label>

        <label className="incident-filter-field">
          <span className="type-metadata">Alpha ID</span>
          <input
            className="incident-filter-input"
            value={draft.alphaId}
            placeholder="alpha-momentum"
            onChange={(event) =>
              setDraft((current) => ({ ...current, alphaId: event.target.value }))
            }
          />
        </label>

        <label className="incident-filter-field">
          <span className="type-metadata">Actor ID</span>
          <input
            className="incident-filter-input"
            value={draft.actorId}
            placeholder="ops-1"
            onChange={(event) =>
              setDraft((current) => ({ ...current, actorId: event.target.value }))
            }
          />
        </label>

        <label className="incident-filter-field">
          <span className="type-metadata">Start timestamp (UTC)</span>
          <input
            className="incident-filter-input"
            type="datetime-local"
            required
            value={draft.startTs}
            onChange={(event) => {
              setDraft((current) => ({
                ...current,
                startTs: event.target.value,
              }));
            }}
          />
        </label>

        <label className="incident-filter-field">
          <span className="type-metadata">End timestamp (UTC)</span>
          <input
            className="incident-filter-input"
            type="datetime-local"
            required
            value={draft.endTs}
            onChange={(event) => {
              setDraft((current) => ({
                ...current,
                endTs: event.target.value,
              }));
            }}
          />
        </label>

        <button className="allocation-form-submit" type="submit">
          Search incidents
        </button>
        <button
          className="allocation-form-submit allocation-form-submit--secondary"
          type="button"
          onClick={() => {
            setState("loading");
            setResult(null);
            setRequestError(null);
            setRefreshTick((value) => value + 1);
          }}
        >
          Refresh timeline
        </button>
      </form>

      {state === "loading" ? (
        <section className="incident-skeleton" role="status" aria-live="polite">
          <p className="type-eyebrow">Loading incident evidence</p>
          <div className="incident-skeleton-line" />
          <div className="incident-skeleton-line incident-skeleton-line--short" />
          <div className="incident-skeleton-line" />
        </section>
      ) : null}

      {state === "empty" ? (
        <section className="incident-empty-state" role="status">
          <p className="type-eyebrow">No incident evidence in this window</p>
          <p className="type-body text-muted">
            Expand time bounds or remove restrictive filters to recover correlated
            trigger/order/fill/PnL evidence.
          </p>
        </section>
      ) : null}

      {requestError && (state === "error" || state === "critical") ? (
        <section className="incident-error-surface" role="alert">
          <p className="type-eyebrow">Incident forensics query failure</p>
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

      {result && result.events.length > 0 ? (
        <>
          <section className="incident-causal-flow">
            <h3 className="type-heading-m">Causal flow</h3>
            <dl className="incident-causal-grid">
              <div>
                <dt className="type-metadata">Trigger</dt>
                <dd className="type-body">{result.causalFlow.trigger}</dd>
              </div>
              <div>
                <dt className="type-metadata">Context</dt>
                <dd className="type-body">{result.causalFlow.context}</dd>
              </div>
              <div>
                <dt className="type-metadata">Action</dt>
                <dd className="type-body">{result.causalFlow.action}</dd>
              </div>
              <div>
                <dt className="type-metadata">Verification</dt>
                <dd className="type-body">{result.causalFlow.verification}</dd>
              </div>
            </dl>
          </section>

          <ol className="incident-timeline-list">
            {result.events.map((eventItem) => (
              <li
                key={eventItem.eventId}
                className="incident-timeline-item"
                data-severity={eventItem.severity}
              >
                <div className="incident-timeline-item-header">
                  <span className="type-eyebrow">{eventItem.stage}</span>
                  <time className="type-mono" dateTime={eventItem.occurredAt}>
                    {eventItem.occurredAt}
                  </time>
                </div>
                <p className="type-body">{eventItem.summary}</p>
                <p className="type-metadata text-muted">
                  Source <code>{eventItem.source}</code> · reason{" "}
                  <code>{eventItem.reasonCode}</code> · correlation{" "}
                  <code>{eventItem.correlationId}</code>
                </p>
                <p className="type-metadata incident-event-action">
                  Next action: {eventItem.recommendedNextAction}
                </p>
              </li>
            ))}
          </ol>
        </>
      ) : null}
    </article>
  );
}
