import { queryOperatorBffReadiness } from "@/lib/workflows/readiness";

interface OperatorWorkflowReadinessCardProps {
  baseUrl: string;
}

export async function OperatorWorkflowReadinessCard({
  baseUrl,
}: OperatorWorkflowReadinessCardProps) {
  const readiness = await queryOperatorBffReadiness({ baseUrl });
  const label =
    readiness.status === "ready"
      ? "Ready"
      : readiness.status === "not_ready"
        ? "Not ready"
        : "Unavailable";

  return (
    <article className="shell-panel shell-panel-grid-item">
      <p className="type-eyebrow">Track A workflow boundary</p>
      <h2 className="type-heading-m">Operator workflow BFF</h2>
      <p className="type-body text-muted">
        Read-only readiness for the TypeScript orchestration layer. Trading, risk,
        research, and governance authority remain in the Rust control plane.
      </p>
      <dl className="shell-state-evidence">
        <div>
          <dt className="type-metadata text-muted">Service state</dt>
          <dd className="type-mono">{label}</dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Auth configured</dt>
          <dd className="type-mono">
            {readiness.authConfigured ? "yes" : "no"}
          </dd>
        </div>
        <div>
          <dt className="type-metadata text-muted">Observed</dt>
          <dd className="type-mono">
            <time dateTime={readiness.observedAtUtc}>{readiness.observedAtUtc}</time>
          </dd>
        </div>
      </dl>
      {readiness.errorCode ? (
        <p className="type-metadata text-muted">{readiness.errorCode}</p>
      ) : null}
    </article>
  );
}
