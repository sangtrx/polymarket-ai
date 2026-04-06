import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

export function ExecutionSummaryCard({
  freshness,
}: {
  freshness: ShellFreshnessSnapshot;
}) {
  return (
    <article className="shell-panel shell-panel-grid-item">
      <div className="shell-panel-header">
        <p className="type-eyebrow">Execution</p>
        <span className="shell-status-pill" data-tone="normal">
          normal
        </span>
      </div>

      <h2 className="type-heading-m">Execution lifecycle shell slot</h2>
      <p className="type-body text-muted">
        Read-only shell placeholder for venue lifecycle and reconciliation
        telemetry.
      </p>

      <dl className="shell-metric-grid">
        <div>
          <dt className="type-metadata">Active orders</dt>
          <dd className="type-mono">137</dd>
        </div>
        <div>
          <dt className="type-metadata">Reconciliation lag</dt>
          <dd className="type-mono">00:00:01.4</dd>
        </div>
      </dl>

      <p className="type-metadata text-muted">
        Last update:{" "}
        <time dateTime={freshness.lastUpdatedIso}>{freshness.lastUpdatedIso}</time>
      </p>
    </article>
  );
}
