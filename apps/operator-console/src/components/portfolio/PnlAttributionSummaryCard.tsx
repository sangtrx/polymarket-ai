"use client";

import { useEffect, useMemo, useState } from "react";
import {
  AttributionClientError,
  queryCostAwareAttribution,
  type AttributionDataState,
  type AttributionPeriod,
  type AttributionQueryResult,
} from "@/lib/portfolio/attribution";
import { PnlAttributionBreakdownTable } from "@/components/portfolio/PnlAttributionBreakdownTable";
import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

interface PnlAttributionSummaryCardProps {
  baseUrl: string;
  freshness: ShellFreshnessSnapshot;
}

type ViewState = AttributionDataState | "loading" | "error" | "critical";

const PERIOD_OPTIONS: AttributionPeriod[] = ["1h", "24h", "30d"];

function toClientError(error: unknown): AttributionClientError {
  if (error instanceof AttributionClientError) {
    return error;
  }
  return new AttributionClientError({
    status: 500,
    errorCode: "attribution_request_failed",
    message:
      error instanceof Error
        ? error.message
        : "Attribution request failed unexpectedly.",
    action: "attribution_query",
    endpoint: "operator-console-ui",
    timestampUtc: new Date().toISOString(),
  });
}

function isCriticalError(error: AttributionClientError): boolean {
  return (
    error.status >= 500 ||
    error.errorCode.includes("projection_unavailable") ||
    error.errorCode.includes("stale_source") ||
    error.errorCode.includes("persistence_unavailable")
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
  if (state === "critical") {
    return "critical";
  }
  if (state === "error") {
    return "error";
  }
  if (state === "empty") {
    return "empty";
  }
  if (state === "loading") {
    return "loading";
  }
  return "ready";
}

function recommendedNextAction(
  state: ViewState,
  result: AttributionQueryResult | null,
): string {
  if (state === "critical") {
    return "Validate reconciliation freshness and projection health before acting on this view.";
  }
  if (state === "error") {
    return "Review filters and retry the attribution query with canonical values.";
  }
  if (result) {
    return result.recommendedNextAction;
  }
  return "Load attribution evidence to reveal the next recommended operator action.";
}

export function PnlAttributionSummaryCard({
  baseUrl,
  freshness,
}: PnlAttributionSummaryCardProps) {
  const [period, setPeriod] = useState<AttributionPeriod>("24h");
  const [marketDraft, setMarketDraft] = useState("");
  const [alphaDraft, setAlphaDraft] = useState("");
  const [marketFilter, setMarketFilter] = useState<string | undefined>(undefined);
  const [alphaFilter, setAlphaFilter] = useState<string | undefined>(undefined);
  const [refreshTick, setRefreshTick] = useState(0);
  const [state, setState] = useState<ViewState>("loading");
  const [result, setResult] = useState<AttributionQueryResult | null>(null);
  const [requestError, setRequestError] = useState<AttributionClientError | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      try {
        const next = await queryCostAwareAttribution({
          baseUrl,
          period,
          marketId: marketFilter,
          alphaId: alphaFilter,
        });
        if (cancelled) {
          return;
        }
        setRequestError(null);
        setResult(next);
        setState(next.dataState);
      } catch (error) {
        if (cancelled) {
          return;
        }
        const clientError = toClientError(error);
        setRequestError(clientError);
        setResult(null);
        setState(isCriticalError(clientError) ? "critical" : "error");
      }
    }

    void run();
    return () => {
      cancelled = true;
    };
  }, [alphaFilter, baseUrl, marketFilter, period, refreshTick]);

  const metadata = useMemo(() => {
    if (!result) {
      return {
        asOfUtc: freshness.lastUpdatedIso,
        source: freshness.source,
        reasonCode:
          state === "critical"
            ? "attribution_stale_source"
            : state === "error"
              ? "attribution_query_failed"
              : "attribution_loading",
        correlationId: requestError?.correlationId ?? "n/a",
        periodStartUtc: freshness.lastUpdatedIso,
        periodEndUtc: freshness.lastUpdatedIso,
      };
    }

    return {
      asOfUtc: result.asOfUtc,
      source: result.source,
      reasonCode: result.reasonCode,
      correlationId: result.correlationId,
      periodStartUtc: result.startInclusiveUtc,
      periodEndUtc: result.endExclusiveUtc,
    };
  }, [freshness.lastUpdatedIso, freshness.source, requestError?.correlationId, result, state]);

  return (
    <section
      className="attribution-summary-card"
      aria-busy={state === "loading"}
      aria-label="Cost-aware PnL attribution"
    >
      <div className="shell-panel-header">
        <div>
          <p className="type-eyebrow">Cost-aware PnL attribution</p>
          <h3 className="type-heading-m">Metadata-first performance evidence</h3>
        </div>
        <span className="shell-status-pill" data-tone={statusTone(state)}>
          {statusLabel(state)}
        </span>
      </div>

      <dl className="attribution-metadata-grid">
        <div>
          <dt className="type-metadata text-muted">As of (UTC)</dt>
          <dd className="type-mono">
            <time dateTime={metadata.asOfUtc}>{metadata.asOfUtc}</time>
          </dd>
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
          <dt className="type-metadata text-muted">Window start (inclusive)</dt>
          <dd className="type-mono">{metadata.periodStartUtc}</dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Window end (exclusive)</dt>
          <dd className="type-mono">{metadata.periodEndUtc}</dd>
        </div>
      </dl>

      <p className="type-body text-muted attribution-narrative">
        Inspect realized/unrealized performance with explicit cost decomposition before
        interpreting strategy-level narrative.
      </p>
      <p className="type-metadata attribution-next-action">
        Recommended next action: {recommendedNextAction(state, result)}
      </p>

      <form
        className="attribution-filter-grid"
        onSubmit={(event) => {
          event.preventDefault();
          setState("loading");
          setRequestError(null);
          setMarketFilter(marketDraft.trim() || undefined);
          setAlphaFilter(alphaDraft.trim() || undefined);
        }}
      >
        <label className="attribution-filter-field">
          <span className="type-metadata">Period</span>
          <select
            className="attribution-filter-input"
            onChange={(event) => {
              setState("loading");
              setRequestError(null);
              setPeriod(event.target.value as AttributionPeriod);
            }}
            value={period}
          >
            {PERIOD_OPTIONS.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>

        <label className="attribution-filter-field">
          <span className="type-metadata">Market filter (optional)</span>
          <input
            className="attribution-filter-input"
            onChange={(event) => setMarketDraft(event.target.value)}
            placeholder="market-btc-election"
            value={marketDraft}
          />
        </label>

        <label className="attribution-filter-field">
          <span className="type-metadata">Alpha filter (optional)</span>
          <input
            className="attribution-filter-input"
            onChange={(event) => setAlphaDraft(event.target.value)}
            placeholder="alpha-momentum"
            value={alphaDraft}
          />
        </label>

        <button className="allocation-form-submit" type="submit">
          Apply filters
        </button>
        <button
          className="allocation-form-submit allocation-form-submit--secondary"
          onClick={() => {
            setState("loading");
            setRequestError(null);
            setRefreshTick((value) => value + 1);
          }}
          type="button"
        >
          Refresh attribution
        </button>
      </form>

      {state === "loading" ? (
        <section className="attribution-skeleton" role="status" aria-live="polite">
          <p className="type-eyebrow">Loading attribution evidence</p>
          <div className="attribution-skeleton-line" />
          <div className="attribution-skeleton-line attribution-skeleton-line--short" />
          <div className="attribution-skeleton-grid">
            <div className="attribution-skeleton-cell" />
            <div className="attribution-skeleton-cell" />
            <div className="attribution-skeleton-cell" />
            <div className="attribution-skeleton-cell" />
          </div>
        </section>
      ) : null}

      {state === "empty" ? (
        <section className="attribution-empty-state" role="status">
          <p className="type-eyebrow">No attribution rows in this window</p>
          <p className="type-body text-muted">
            The selected period produced zero activity. Adjust period, market, or alpha filters
            to reveal attributable performance.
          </p>
        </section>
      ) : null}

      {state === "error" || state === "critical" ? (
        <section className="attribution-error-surface" role="alert">
          <p className="type-eyebrow">Attribution query failure</p>
          <p className="type-body text-muted">{requestError?.message ?? "Unknown failure."}</p>
          <p className="type-metadata">
            Error code: <code>{requestError?.errorCode ?? "attribution_unknown_error"}</code>
          </p>
          <p className="type-metadata">
            Action: {requestError?.action ?? "attribution_query"} · correlation ID:{" "}
            {requestError?.correlationId ?? "n/a"}
          </p>
        </section>
      ) : null}

      {state === "ready" && result ? <PnlAttributionBreakdownTable rows={result.rows} /> : null}
    </section>
  );
}
