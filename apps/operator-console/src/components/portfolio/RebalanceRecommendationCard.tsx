"use client";

import { useState } from "react";
import {
  AllocationPolicyClientError,
  evaluateRebalanceRecommendation,
  executeRebalanceRecommendation,
  listPendingRebalanceRecommendations,
  type PendingRebalanceRecommendationItem,
  type RebalanceRecommendationDecision,
} from "@/lib/portfolio/allocation-policy";

interface RebalanceRecommendationCardProps {
  baseUrl: string;
}

function toClientError(error: unknown): AllocationPolicyClientError {
  if (error instanceof AllocationPolicyClientError) {
    return error;
  }
  return new AllocationPolicyClientError({
    status: 500,
    errorCode: "rebalance_request_failed",
    message:
      error instanceof Error
        ? error.message
        : "Rebalance workflow request failed unexpectedly.",
    action: "rebalance_recommendation_evaluate",
    endpoint: "operator-console-ui",
    timestampUtc: new Date().toISOString(),
  });
}

export function RebalanceRecommendationCard({
  baseUrl,
}: RebalanceRecommendationCardProps) {
  const [policyKey, setPolicyKey] = useState("portfolio-default");
  const [exposureDriftPct, setExposureDriftPct] = useState("12");
  const [relativeAlphaDriftPct, setRelativeAlphaDriftPct] = useState("11");
  const [requireExecution, setRequireExecution] = useState(false);
  const [approvalRequestId, setApprovalRequestId] = useState("");
  const [recommendationId, setRecommendationId] = useState("");
  const [decision, setDecision] = useState<RebalanceRecommendationDecision | null>(
    null,
  );
  const [pendingRecommendations, setPendingRecommendations] = useState<
    PendingRebalanceRecommendationItem[]
  >([]);
  const [requestError, setRequestError] = useState<AllocationPolicyClientError | null>(
    null,
  );
  const [isEvaluating, setIsEvaluating] = useState(false);
  const [isExecuting, setIsExecuting] = useState(false);
  const [isLoadingPending, setIsLoadingPending] = useState(false);

  const canEvaluate =
    policyKey.trim().length > 0 &&
    Number.isFinite(Number(exposureDriftPct)) &&
    Number.isFinite(Number(relativeAlphaDriftPct));

  const executeTargetId =
    recommendationId.trim() || decision?.recommendationId || "";

  const showRecommendedAction =
    decision?.recommendedNextAction ?? "Evaluate drift to reveal next action guidance.";

  return (
    <section
      className="rebalance-recommendation-card"
      aria-label="Rebalance recommendation review"
    >
      <div className="shell-panel-header">
        <div>
          <p className="type-eyebrow">Rebalance recommendation</p>
          <h3 className="type-heading-m">Drift rationale + approval context</h3>
        </div>
        <span
          className="shell-status-pill"
          data-tone={
            requestError
              ? "critical"
              : decision?.approvalStatus === "pending"
                ? "warning"
                : "normal"
          }
        >
          {requestError
            ? "error"
            : decision?.approvalStatus === "pending"
              ? "pending"
              : "ready"}
        </span>
      </div>

      <p className="type-body text-muted">
        Review deterministic drift evidence before execution and keep approval
        context explicit for critical paths.
      </p>
      <p className="type-metadata">
        Recommended next action: {showRecommendedAction}
      </p>

      <div className="allocation-form-grid">
        <label className="allocation-form-field">
          <span className="type-metadata">Policy key</span>
          <input
            className="allocation-form-input"
            onChange={(event) => setPolicyKey(event.target.value)}
            value={policyKey}
          />
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Exposure drift (%)</span>
          <input
            className="allocation-form-input"
            inputMode="decimal"
            onChange={(event) => setExposureDriftPct(event.target.value)}
            value={exposureDriftPct}
          />
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Relative alpha drift (%)</span>
          <input
            className="allocation-form-input"
            inputMode="decimal"
            onChange={(event) => setRelativeAlphaDriftPct(event.target.value)}
            value={relativeAlphaDriftPct}
          />
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Approval request ID (optional)</span>
          <input
            className="allocation-form-input"
            onChange={(event) => setApprovalRequestId(event.target.value)}
            value={approvalRequestId}
          />
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Execute through control path</span>
          <input
            checked={requireExecution}
            className="allocation-form-checkbox"
            onChange={(event) => setRequireExecution(event.target.checked)}
            type="checkbox"
          />
        </label>
      </div>

      <div className="rebalance-control-row">
        <button
          className="allocation-form-submit"
          disabled={!canEvaluate || isEvaluating}
          onClick={async () => {
            if (!canEvaluate || !baseUrl.trim()) {
              return;
            }
            setIsEvaluating(true);
            setRequestError(null);
            try {
              const nextDecision = await evaluateRebalanceRecommendation({
                baseUrl,
                policyKey: policyKey.trim(),
                exposureDriftPct: Number(exposureDriftPct),
                relativeAlphaDriftPct: Number(relativeAlphaDriftPct),
                requireExecution,
                approvalRequestId: approvalRequestId.trim() || undefined,
              });
              setDecision(nextDecision);
              setRecommendationId(nextDecision.recommendationId);
            } catch (error) {
              setRequestError(toClientError(error));
            } finally {
              setIsEvaluating(false);
            }
          }}
          type="button"
        >
          {isEvaluating ? "Evaluating drift…" : "Evaluate drift"}
        </button>

        <button
          className="allocation-form-submit allocation-form-submit--secondary"
          disabled={isLoadingPending || !baseUrl.trim()}
          onClick={async () => {
            if (!baseUrl.trim()) {
              return;
            }
            setIsLoadingPending(true);
            setRequestError(null);
            try {
              const pending = await listPendingRebalanceRecommendations({
                baseUrl,
                policyKey: policyKey.trim() || undefined,
              });
              setPendingRecommendations(pending.pendingRecommendations);
            } catch (error) {
              setRequestError(toClientError(error));
            } finally {
              setIsLoadingPending(false);
            }
          }}
          type="button"
        >
          {isLoadingPending ? "Refreshing queue…" : "Load pending recommendations"}
        </button>
      </div>

      <div className="rebalance-control-row">
        <label className="allocation-form-field allocation-form-field--inline">
          <span className="type-metadata">Recommendation ID for execution</span>
          <input
            className="allocation-form-input"
            onChange={(event) => setRecommendationId(event.target.value)}
            value={recommendationId}
          />
        </label>
        <button
          className="allocation-form-submit"
          disabled={!executeTargetId || isExecuting || !baseUrl.trim()}
          onClick={async () => {
            if (!executeTargetId || !baseUrl.trim()) {
              return;
            }
            setIsExecuting(true);
            setRequestError(null);
            try {
              const executed = await executeRebalanceRecommendation({
                baseUrl,
                recommendationId: executeTargetId,
                approvalRequestId: approvalRequestId.trim() || undefined,
              });
              setDecision(executed);
              setRecommendationId(executed.recommendationId);
            } catch (error) {
              setRequestError(toClientError(error));
            } finally {
              setIsExecuting(false);
            }
          }}
          type="button"
        >
          {isExecuting ? "Executing recommendation…" : "Execute recommendation"}
        </button>
      </div>

      {decision ? (
        <section className="allocation-evidence" role="status">
          <p className="type-eyebrow">Recommendation evidence</p>
          <dl className="risk-evidence-grid">
            <div>
              <dt className="type-metadata text-muted">Recommendation status</dt>
              <dd className="type-mono">{decision.recommendationStatus}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Approval context</dt>
              <dd className="type-mono">{decision.approvalStatus}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Reason code</dt>
              <dd className="type-mono">
                <code>{decision.reasonCode}</code>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Action type</dt>
              <dd className="type-mono">{decision.actionType}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Correlation ID</dt>
              <dd className="type-mono">{decision.correlationId}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Approval reference</dt>
              <dd className="type-mono">{decision.approvalReference ?? "n/a"}</dd>
            </div>
          </dl>
          <p className="type-body text-muted">{decision.rationale}</p>
          <p className="type-metadata">
            Recommended next action: {decision.recommendedNextAction}
          </p>
        </section>
      ) : null}

      {pendingRecommendations.length > 0 ? (
        <section className="allocation-evidence" role="status">
          <p className="type-eyebrow">Pending recommendation queue</p>
          <ul className="rebalance-pending-list">
            {pendingRecommendations.map((item) => (
              <li key={`${item.recommendationId}:${item.correlationId}`}>
                <p className="type-metadata">
                  <code>{item.recommendationId}</code> — {item.recommendationStatus} (
                  {item.approvalStatus})
                </p>
                <p className="type-body text-muted">{item.rationale}</p>
                <p className="type-metadata">
                  Recommended next action: {item.recommendedNextAction}
                </p>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {requestError ? (
        <section className="allocation-form-error-surface" role="alert">
          <p className="type-eyebrow">Rebalance workflow failure</p>
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
    </section>
  );
}
