import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

export function RiskPostureCard({
  freshness,
}: {
  freshness: ShellFreshnessSnapshot;
}) {
  return (
    <article className="shell-panel shell-panel-grid-item">
      <div className="shell-panel-header">
        <p className="type-eyebrow">Risk</p>
        <span
          className="shell-status-pill"
          data-tone={freshness.isStale ? "critical" : "warning"}
        >
          {freshness.isStale ? "stale" : "warning"}
        </span>
      </div>

      <h2 className="type-heading-m">Risk posture shell slot</h2>
      <p className="type-body text-muted">
        Read-only shell placeholder for safe-state posture and market exposure
        drift.
      </p>

      <dl className="shell-metric-grid">
        <div>
          <dt className="type-metadata">Aggregate exposure</dt>
          <dd className="type-mono">42.1%</dd>
        </div>
        <div>
          <dt className="type-metadata">Critical limit breaches</dt>
          <dd className="type-mono">2</dd>
        </div>
      </dl>

      <p className="type-metadata text-muted">
        Last update:{" "}
        <time dateTime={freshness.lastUpdatedIso}>{freshness.lastUpdatedIso}</time>
      </p>
    </article>
  );
}
