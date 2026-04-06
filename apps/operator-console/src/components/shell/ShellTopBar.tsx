import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

interface ShellTopBarProps {
  title: string;
  description: string;
  freshness: ShellFreshnessSnapshot;
  p95TargetMs: number;
}

export function ShellTopBar({
  title,
  description,
  freshness,
  p95TargetMs,
}: ShellTopBarProps) {
  return (
    <div className="shell-top-bar">
      <div>
        <p className="type-eyebrow">Operator console shell</p>
        <h1 className="type-heading-xl">{title}</h1>
        <p className="type-body text-muted">{description}</p>
      </div>

      <div className="shell-top-bar-meta">
        <div>
          <p className="type-metadata text-muted">Freshness evidence</p>
          <p className="type-mono">
            <time dateTime={freshness.lastUpdatedIso}>
              {freshness.lastUpdatedIso}
            </time>
          </p>
          <p className="type-metadata text-muted">Source: {freshness.source}</p>
        </div>

        <div>
          <p className="type-metadata text-muted">State</p>
          <p>
            <span
              className="shell-freshness-pill"
              data-stale={freshness.isStale ? "true" : "false"}
            >
              {freshness.isStale ? "Stale" : "Fresh"}
            </span>
          </p>
          <p className="type-metadata text-muted">
            p95 read-model budget: {p95TargetMs}ms
          </p>
        </div>

        <div>
          <p className="type-metadata text-muted">
            Mobile policy: monitor-first and action-limited
          </p>
          <button
            aria-disabled="true"
            className="shell-action-disabled"
            disabled
            type="button"
          >
            Destructive controls disabled by default
          </button>
        </div>
      </div>
    </div>
  );
}
