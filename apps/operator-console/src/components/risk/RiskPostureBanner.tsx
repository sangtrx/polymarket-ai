import type { RiskPostureViewModel } from "@/lib/risk/posture";

const POSTURE_LABELS: Record<RiskPostureViewModel["posture"], string> = {
  normal: "normal",
  warning: "warning",
  critical: "critical",
  "locked-safe": "locked-safe",
};

export function RiskPostureBanner({ model }: { model: RiskPostureViewModel }) {
  const announcementSummaryId = "risk-banner-announcement-summary";

  return (
    <section
      aria-describedby={announcementSummaryId}
      aria-atomic="true"
      aria-live={model.ariaLive}
      className="risk-banner"
      data-posture={model.posture}
      role="status"
    >
      <p className="assistive-announcement risk-banner--announcement" id={announcementSummaryId}>
        Outcome: {model.evidence.resultingMode}. Timestamp: {model.evidence.lastUpdatedIso}.
        Reason: {model.evidence.reasonCode}. Action ID: {model.evidence.actionId ?? "n/a"}.
        Correlation ID: {model.evidence.correlationId ?? "n/a"}.
      </p>
      <div className="shell-panel-header">
        <div>
          <p className="type-eyebrow">Risk posture banner</p>
          <h2 className="type-heading-m">{model.headline}</h2>
        </div>
        <span className="shell-status-pill" data-tone={model.posture}>
          {POSTURE_LABELS[model.posture]}
        </span>
      </div>

      <p className="type-body">{model.summary}</p>
      <p className="risk-required-action type-body">
        <strong>Recommended next action:</strong> {model.recommendedNextAction}
      </p>
      <p className="type-metadata text-muted">{model.requiredActionGuidance}</p>

      <dl className="risk-evidence-grid">
        <div>
          <dt className="type-metadata text-muted">Last update</dt>
          <dd className="type-mono">
            <time dateTime={model.evidence.lastUpdatedIso}>
              {model.evidence.lastUpdatedIso}
            </time>
          </dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Source</dt>
          <dd className="type-mono">{model.evidence.source}</dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Actor/source</dt>
          <dd className="type-mono">{model.evidence.actorSource}</dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Resulting mode</dt>
          <dd className="type-mono">{model.evidence.resultingMode}</dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Reason code</dt>
          <dd className="type-mono">
            <code>{model.evidence.reasonCode}</code>
          </dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Action ID</dt>
          <dd className="type-mono">{model.evidence.actionId ?? "n/a"}</dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Correlation ID</dt>
          <dd className="type-mono">{model.evidence.correlationId ?? "n/a"}</dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Audit reference</dt>
          <dd className="type-mono">{model.evidence.auditReference ?? "n/a"}</dd>
        </div>
      </dl>
    </section>
  );
}
