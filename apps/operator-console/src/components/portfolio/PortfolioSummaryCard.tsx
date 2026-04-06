import { AllocationPolicyForm } from "@/components/portfolio/AllocationPolicyForm";
import { PnlAttributionSummaryCard } from "@/components/portfolio/PnlAttributionSummaryCard";
import { RebalanceRecommendationCard } from "@/components/portfolio/RebalanceRecommendationCard";
import { getOperatorConsoleEnv } from "@/lib/env";
import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

export function PortfolioSummaryCard({
  freshness,
}: {
  freshness: ShellFreshnessSnapshot;
}) {
  const { apiBaseUrl } = getOperatorConsoleEnv();

  return (
    <article className="shell-panel shell-panel-grid-item">
      <div className="shell-panel-header">
        <p className="type-eyebrow">Portfolio</p>
        <span className="shell-status-pill" data-tone="normal">
          normal
        </span>
      </div>

      <h2 className="type-heading-m">Portfolio summary shell slot</h2>
      <p className="type-body text-muted">
        Cost-aware realized/unrealized attribution plus allocation/rebalance controls for operator
        triage.
      </p>

      <dl className="shell-metric-grid">
        <div>
          <dt className="type-metadata">Market value</dt>
          <dd className="type-mono">$2.48M</dd>
        </div>
        <div>
          <dt className="type-metadata">Available allocation</dt>
          <dd className="type-mono">17.6%</dd>
        </div>
      </dl>

      <p className="type-metadata text-muted">
        Last update:{" "}
        <time dateTime={freshness.lastUpdatedIso}>{freshness.lastUpdatedIso}</time>
      </p>

      <PnlAttributionSummaryCard baseUrl={apiBaseUrl} freshness={freshness} />

      <section className="portfolio-allocation-grid">
        <AllocationPolicyForm baseUrl={apiBaseUrl} />
        <RebalanceRecommendationCard baseUrl={apiBaseUrl} />
      </section>
    </article>
  );
}
