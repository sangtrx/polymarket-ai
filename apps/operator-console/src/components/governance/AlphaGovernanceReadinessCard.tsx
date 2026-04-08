"use client";

import { useEffect, useMemo, useState } from "react";
import {
  GovernanceReadinessClientError,
  queryAlphaGovernanceReadiness,
  type AlphaGovernanceReadinessResult,
} from "@/lib/governance/readiness";
import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

export interface AlphaGovernanceReadinessCardProps {
  baseUrl: string;
  freshness: ShellFreshnessSnapshot;
  candidateId?: string;
  alphaId?: string;
}

type ViewState = "loading" | "ready" | "empty" | "error" | "critical";

function toClientError(error: unknown): GovernanceReadinessClientError {
  if (error instanceof GovernanceReadinessClientError) {
    return error;
  }
  return new GovernanceReadinessClientError({
    status: 500,
    errorCode: "governance_readiness_request_failed",
    message:
      error instanceof Error
        ? error.message
        : "Governance readiness request failed unexpectedly.",
    action: "governance_readiness_query",
    endpoint: "operator-console-ui",
    timestampUtc: new Date().toISOString(),
  });
}

function isCriticalError(error: GovernanceReadinessClientError): boolean {
  return (
    error.status >= 500 ||
    error.errorCode.includes("dependency_unavailable") ||
    error.errorCode.includes("state_unavailable") ||
    error.errorCode.includes("persistence_unavailable")
  );
}

function statusTone(
  state: ViewState,
  result: AlphaGovernanceReadinessResult | null,
): "normal" | "warning" | "critical" {
  if (state === "critical") {
    return "critical";
  }
  if (
    state === "loading" ||
    state === "empty" ||
    state === "error" ||
    result?.readinessStatus === "blocked"
  ) {
    return "warning";
  }
  return "normal";
}

function statusLabel(
  state: ViewState,
  result: AlphaGovernanceReadinessResult | null,
): string {
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
  if (result?.readinessStatus === "blocked") {
    return "blocked";
  }
  return "ready";
}

function recommendedAction(
  state: ViewState,
  result: AlphaGovernanceReadinessResult | null,
): string {
  if (result) {
    return result.recommendedNextAction;
  }
  if (state === "critical") {
    return "Validate research dependency health and restore machine-readable evidence before governance actions.";
  }
  if (state === "error") {
    return "Correct candidate_id/alpha_id values and retry with canonical identifiers.";
  }
  if (state === "empty") {
    return "Provide candidate_id and alpha_id to compute readiness and reveal blocked/ready evidence.";
  }
  return "Loading governance readiness evidence.";
}

function normalizeDraft(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

export function AlphaGovernanceReadinessCard({
  baseUrl,
  freshness,
  candidateId,
  alphaId,
}: AlphaGovernanceReadinessCardProps) {
  const [candidateDraft, setCandidateDraft] = useState(candidateId ?? "");
  const [alphaDraft, setAlphaDraft] = useState(alphaId ?? "");
  const [candidateFilter, setCandidateFilter] = useState(candidateId);
  const [alphaFilter, setAlphaFilter] = useState(alphaId);
  const [refreshTick, setRefreshTick] = useState(0);
  const [state, setState] = useState<ViewState>(
    candidateId?.trim() && alphaId?.trim() ? "loading" : "empty",
  );
  const [result, setResult] = useState<AlphaGovernanceReadinessResult | null>(null);
  const [requestError, setRequestError] = useState<GovernanceReadinessClientError | null>(
    null,
  );
  const [lastAnnouncementAtUtc, setLastAnnouncementAtUtc] = useState(
    freshness.lastUpdatedIso,
  );

  useEffect(() => {
    let cancelled = false;

    const normalizedCandidate = normalizeDraft(candidateFilter ?? "");
    const normalizedAlpha = normalizeDraft(alphaFilter ?? "");
    if (!normalizedCandidate || !normalizedAlpha) {
      return () => {
        cancelled = true;
      };
    }
    const queryCandidateId = normalizedCandidate;
    const queryAlphaId = normalizedAlpha;

    async function run() {
      setState("loading");
      try {
        const next = await queryAlphaGovernanceReadiness({
          baseUrl,
          candidateId: queryCandidateId,
          alphaId: queryAlphaId,
        });
        if (cancelled) {
          return;
        }
        setResult(next);
        setRequestError(null);
        setState("ready");
        setLastAnnouncementAtUtc(next.asOfUtc);
      } catch (error) {
        if (cancelled) {
          return;
        }
        const clientError = toClientError(error);
        setResult(null);
        setRequestError(clientError);
        setState(isCriticalError(clientError) ? "critical" : "error");
        setLastAnnouncementAtUtc(clientError.timestampUtc);
      }
    }

    void run();
    return () => {
      cancelled = true;
    };
  }, [alphaFilter, baseUrl, candidateFilter, refreshTick]);

  const metadata = useMemo(() => {
    if (result) {
      return {
        asOfUtc: result.asOfUtc,
        source: result.source,
        reasonCode: result.reasonCode,
        correlationId: result.correlationId,
        status: result.readinessStatus,
        candidateId: result.candidateId,
        alphaId: result.alphaId,
      };
    }

    return {
      asOfUtc: freshness.lastUpdatedIso,
      source: freshness.source,
      reasonCode:
        state === "critical"
          ? requestError?.errorCode ?? "governance_readiness_dependency_unavailable"
          : state === "error"
            ? requestError?.errorCode ?? "governance_readiness_invalid_payload"
            : state === "empty"
              ? "governance_readiness_inputs_missing"
              : "governance_readiness_loading",
      correlationId: requestError?.correlationId ?? "n/a",
      status: state === "loading" ? "loading" : state,
      candidateId: normalizeDraft(candidateFilter ?? "") ?? "n/a",
      alphaId: normalizeDraft(alphaFilter ?? "") ?? "n/a",
    };
  }, [
    alphaFilter,
    candidateFilter,
    freshness.lastUpdatedIso,
    freshness.source,
    requestError?.correlationId,
    requestError?.errorCode,
    result,
    state,
  ]);

  return (
    <article
      className="shell-panel shell-panel-grid-item governance-readiness-card"
      aria-busy={state === "loading"}
      aria-label="Alpha governance readiness card"
    >
      <div className="shell-panel-header">
        <div>
          <p className="type-eyebrow">Alpha governance readiness</p>
          <h2 className="type-heading-m">Lifecycle and validation readiness evidence</h2>
        </div>
        <span className="shell-status-pill" data-tone={statusTone(state, result)}>
          {statusLabel(state, result)}
        </span>
      </div>

      <dl className="governance-readiness-metadata-grid">
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
          <dt className="type-metadata text-muted">Readiness status</dt>
          <dd className="type-mono">
            <code>{metadata.status}</code>
          </dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Candidate / Alpha</dt>
          <dd className="type-mono">
            <code>
              {metadata.candidateId} / {metadata.alphaId}
            </code>
          </dd>
        </div>
      </dl>

      <p className="type-metadata governance-readiness-next-action" data-tone={statusTone(state, result)}>
        Recommended next action: {recommendedAction(state, result)}
      </p>

      <section
        className="governance-readiness-announcement"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        <p className="type-metadata">
          Status <code>{statusLabel(state, result)}</code> at{" "}
          <time dateTime={lastAnnouncementAtUtc}>{lastAnnouncementAtUtc}</time>.
        </p>
      </section>

      <form
        className="governance-readiness-filter-grid"
        onSubmit={(event) => {
          event.preventDefault();
          const nextCandidate = normalizeDraft(candidateDraft);
          const nextAlpha = normalizeDraft(alphaDraft);
          setRequestError(null);
          setResult(null);
          if (!nextCandidate || !nextAlpha) {
            setState("empty");
          }
          setCandidateFilter(candidateDraft);
          setAlphaFilter(alphaDraft);
        }}
      >
        <label className="governance-readiness-filter-field">
          <span className="type-metadata">candidate_id</span>
          <input
            className="governance-readiness-filter-input"
            value={candidateDraft}
            placeholder="candidate::alpha-1"
            onChange={(event) => setCandidateDraft(event.target.value)}
          />
        </label>
        <label className="governance-readiness-filter-field">
          <span className="type-metadata">alpha_id</span>
          <input
            className="governance-readiness-filter-input"
            value={alphaDraft}
            placeholder="alpha::mean-reversion"
            onChange={(event) => setAlphaDraft(event.target.value)}
          />
        </label>
        <button className="allocation-form-submit" type="submit">
          Load readiness
        </button>
        <button
          className="allocation-form-submit allocation-form-submit--secondary"
          type="button"
          onClick={() => {
            setRequestError(null);
            setResult(null);
            setRefreshTick((value) => value + 1);
          }}
        >
          Refresh readiness
        </button>
      </form>

      {state === "loading" ? (
        <section className="governance-readiness-skeleton" role="status" aria-live="polite">
          <p className="type-eyebrow">Loading governance readiness evidence</p>
          <div className="governance-readiness-skeleton-line" />
          <div className="governance-readiness-skeleton-line governance-readiness-skeleton-line--short" />
          <div className="governance-readiness-skeleton-grid">
            <div className="governance-readiness-skeleton-cell" />
            <div className="governance-readiness-skeleton-cell" />
            <div className="governance-readiness-skeleton-cell" />
          </div>
        </section>
      ) : null}

      {state === "empty" ? (
        <section className="governance-readiness-empty-state" role="status">
          <p className="type-eyebrow">Readiness inputs are required</p>
          <p className="type-body text-muted">
            Provide canonical <code>candidate_id</code> and <code>alpha_id</code> to
            compute lifecycle, completeness, shadow stability, and guardrail readiness.
          </p>
        </section>
      ) : null}

      {requestError && (state === "error" || state === "critical") ? (
        <section className="governance-readiness-error-surface" role="alert">
          <p className="type-eyebrow">Governance readiness query failure</p>
          <p className="type-body text-muted">{requestError.message}</p>
          <p className="type-metadata">
            Error code: <code>{requestError.errorCode}</code>
          </p>
          <p className="type-metadata">
            Action: {requestError.action} · endpoint:{" "}
            <code>{requestError.endpoint}</code> · correlation ID:{" "}
            {requestError.correlationId ?? "n/a"}
          </p>
        </section>
      ) : null}

      {state === "ready" && result ? (
        <section className="governance-readiness-details">
          <div className="governance-readiness-signal-grid">
            <article className="governance-readiness-signal">
              <p className="type-eyebrow">Lifecycle</p>
              <p className="type-body">
                <code>{result.lifecycleState}</code>
              </p>
            </article>
            <article className="governance-readiness-signal">
              <p className="type-eyebrow">Validation completeness</p>
              <p className="type-body">
                <code>{result.validationCompleteness}</code>
              </p>
            </article>
            <article className="governance-readiness-signal">
              <p className="type-eyebrow">Shadow stability</p>
              <p className="type-body">
                <code>{result.shadowStability}</code>
              </p>
            </article>
            <article className="governance-readiness-signal">
              <p className="type-eyebrow">Guardrail state</p>
              <p className="type-body">
                <code>{result.guardrailState}</code>
              </p>
            </article>
          </div>

          {result.readinessStatus === "blocked" ? (
            <section className="governance-readiness-blocked-surface">
              <p className="type-eyebrow">Blocked readiness artifacts</p>
              <ul className="governance-readiness-blocked-list">
                {result.missingArtifacts.map((artifact) => (
                  <li key={artifact}>
                    <code>{artifact}</code>
                  </li>
                ))}
              </ul>
            </section>
          ) : (
            <section className="governance-readiness-ready-surface">
              <p className="type-eyebrow">Readiness signal</p>
              <p className="type-body">
                <code>{"readinessStatus === \"ready\""}</code>
              </p>
            </section>
          )}

          <section className="governance-readiness-window-surface">
            <p className="type-eyebrow">FR10 guardrail windows</p>
            <p className="type-metadata text-muted">
              Required windows: <code>1h</code>, <code>24h</code>, <code>30d</code>.
            </p>
            <p className="type-metadata text-muted">
              Boundary semantics: {result.guardrailWindows[0]?.boundarySemantics}
            </p>
            <div className="governance-readiness-window-grid">
              {result.guardrailWindows.map((windowEvidence) => (
                <article
                  key={windowEvidence.window}
                  className="governance-readiness-window-item"
                >
                  <p className="type-heading-m">{windowEvidence.window}</p>
                  <p className="type-metadata">
                    status:{" "}
                    <code>
                      {windowEvidence.available ? "available" : "missing"}
                    </code>
                  </p>
                  {windowEvidence.available ? (
                    <p className="type-metadata">
                      net_pnl: <code>{windowEvidence.netPnl}</code> · sharpe:{" "}
                      <code>{windowEvidence.rollingSharpe}</code>
                    </p>
                  ) : null}
                </article>
              ))}
            </div>
          </section>
        </section>
      ) : null}
    </article>
  );
}
