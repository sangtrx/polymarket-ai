import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

export function IncidentTimelineCard({
  freshness,
}: {
  freshness: ShellFreshnessSnapshot;
}) {
  return (
    <article className="shell-panel shell-panel-grid-item">
      <div className="shell-panel-header">
        <p className="type-eyebrow">Timeline</p>
        <span
          className="shell-status-pill"
          data-tone={freshness.isStale ? "critical" : "normal"}
        >
          {freshness.isStale ? "stale" : "normal"}
        </span>
      </div>

      <h2 className="type-heading-m">Incident timeline shell slot</h2>
      <p className="type-body text-muted">
        Read-only shell placeholder for timestamped operational events and
        alert milestones.
      </p>

      <dl className="shell-metric-grid">
        <div>
          <dt className="type-metadata">Open incidents</dt>
          <dd className="type-mono">3</dd>
        </div>
        <div>
          <dt className="type-metadata">Newest event lag</dt>
          <dd className="type-mono">00:00:00.9</dd>
        </div>
      </dl>

      <p className="type-metadata text-muted">
        Last update:{" "}
        <time dateTime={freshness.lastUpdatedIso}>{freshness.lastUpdatedIso}</time>
      </p>
    </article>
  );
}
