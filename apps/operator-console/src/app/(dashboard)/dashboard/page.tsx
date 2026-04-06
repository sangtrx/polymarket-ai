import { ExecutionSummaryCard } from "@/components/execution/ExecutionSummaryCard";
import { GovernanceQueueCard } from "@/components/governance/GovernanceQueueCard";
import { PortfolioSummaryCard } from "@/components/portfolio/PortfolioSummaryCard";
import { InPageTabs } from "@/components/shell/InPageTabs";
import { OperatorShellLayout } from "@/components/shell/OperatorShellLayout";
import { ShellStatePanel } from "@/components/shell/ShellStatePanel";
import { IncidentTimelineCard } from "@/components/timeline/IncidentTimelineCard";
import { resolveRiskPostureViewModel } from "@/lib/risk/posture";
import {
  type ShellSearchParams,
  resolveShellReadModel,
} from "@/lib/shell/read-models";

interface DashboardPageProps {
  searchParams?: Promise<ShellSearchParams>;
}

export default async function DashboardPage({ searchParams }: DashboardPageProps) {
  const resolvedSearchParams = (await searchParams) ?? {};
  const stateModel = resolveShellReadModel(
    resolvedSearchParams,
    "dashboard.read-model.shell",
  );
  const riskPosture = resolveRiskPostureViewModel(resolvedSearchParams, stateModel);

  const tabViews = [
    {
      id: "overview",
      label: "Overview",
      content: (
        <section className="shell-panel-grid">
          <PortfolioSummaryCard freshness={stateModel.freshness} />
          <ExecutionSummaryCard freshness={stateModel.freshness} />
          <GovernanceQueueCard freshness={stateModel.freshness} />
          <IncidentTimelineCard freshness={stateModel.freshness} />
        </section>
      ),
    },
    {
      id: "risk-execution",
      label: "Risk + Execution",
      content: (
        <section className="shell-panel-grid">
          <PortfolioSummaryCard freshness={stateModel.freshness} />
          <ExecutionSummaryCard freshness={stateModel.freshness} />
          <article className="shell-panel shell-panel-grid-item">
            <p className="type-eyebrow">NFR1 readiness</p>
            <h2 className="type-heading-m">p95 query budget contract</h2>
            <p className="type-body text-muted">
              Dashboard shell slots are wired to explicit read-model evidence and
              retain deterministic fallback behavior if responses exceed{" "}
              {stateModel.p95TargetMs}
              ms.
            </p>
          </article>
        </section>
      ),
    },
    {
      id: "health",
      label: "Freshness evidence",
      content: (
        <article className="shell-panel">
          <p className="type-eyebrow">FR26 evidence</p>
          <h2 className="type-heading-m">Shell freshness traceability</h2>
          <p className="type-body text-muted">
            This shell exposes deterministic freshness evidence for review and
            incident response. Route-level fallback states always include
            machine-readable error context and timestamp.
          </p>
          <dl className="shell-state-evidence">
            <div>
              <dt className="type-metadata text-muted">Last update</dt>
              <dd className="type-mono">
                <time dateTime={stateModel.freshness.lastUpdatedIso}>
                  {stateModel.freshness.lastUpdatedIso}
                </time>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Source</dt>
              <dd className="type-mono">{stateModel.freshness.source}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Stale indicator</dt>
              <dd className="type-mono">
                {stateModel.freshness.isStale ? "stale" : "fresh"}
              </dd>
            </div>
          </dl>
        </article>
      ),
    },
  ] as const;

  return (
    <OperatorShellLayout
      activeRoute="dashboard"
      description="Risk, execution, and governance shell surfaces with deterministic fallback evidence."
      freshness={stateModel.freshness}
      p95TargetMs={stateModel.p95TargetMs}
      riskPosture={riskPosture}
      title="Portfolio command center"
    >
      {stateModel.dataState === "ready" ? (
        <InPageTabs
          ariaLabel="Dashboard workflow views"
          queryKey="view"
          tabs={tabViews}
        />
      ) : (
        <ShellStatePanel stateModel={stateModel} />
      )}
    </OperatorShellLayout>
  );
}
