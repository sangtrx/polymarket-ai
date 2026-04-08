import { GovernanceQueueCard } from "@/components/governance/GovernanceQueueCard";
import { InPageTabs } from "@/components/shell/InPageTabs";
import { OperatorShellLayout } from "@/components/shell/OperatorShellLayout";
import { ShellStatePanel } from "@/components/shell/ShellStatePanel";
import { getOperatorConsoleEnv } from "@/lib/env";
import { resolveRiskPostureViewModel } from "@/lib/risk/posture";
import {
  type ShellSearchParams,
  resolveShellReadModel,
} from "@/lib/shell/read-models";

interface GovernancePageProps {
  searchParams?: Promise<ShellSearchParams>;
}

export default async function GovernancePage({
  searchParams,
}: GovernancePageProps) {
  const resolvedSearchParams = (await searchParams) ?? {};
  const stateModel = resolveShellReadModel(
    resolvedSearchParams,
    "governance.read-model.shell",
  );
  const { apiBaseUrl } = getOperatorConsoleEnv();
  const riskPosture = resolveRiskPostureViewModel(resolvedSearchParams, stateModel);

  const tabViews = [
    {
      id: "approvals",
      label: "Approvals",
      content: (
        <section className="shell-panel-grid">
          <GovernanceQueueCard
            baseUrl={apiBaseUrl}
            freshness={stateModel.freshness}
          />
          <article className="shell-panel shell-panel-grid-item">
            <p className="type-eyebrow">Control boundary</p>
            <h2 className="type-heading-m">
              Safety actions are governed with explicit evidence
            </h2>
            <p className="type-body text-muted">
              Governance surfaces remain approval-centric while emergency safety
              mutations execute through persistent rail confirmations and audit
              metadata.
            </p>
          </article>
        </section>
      ),
    },
    {
      id: "audit",
      label: "Audit readiness",
      content: (
        <article className="shell-panel">
          <p className="type-eyebrow">Audit traceability</p>
          <h2 className="type-heading-m">Canonical shell evidence</h2>
          <p className="type-body text-muted">
            Governance shell views expose ISO-8601 UTC freshness evidence and
            explicit fallback state messaging for review and incident QA.
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
          </dl>
        </article>
      ),
    },
  ] as const;

  return (
    <OperatorShellLayout
      activeRoute="governance"
      description="Approval and audit shell views with explicit non-destructive fallback states."
      freshness={stateModel.freshness}
      p95TargetMs={stateModel.p95TargetMs}
      riskPosture={riskPosture}
      title="Governance control plane"
    >
      {stateModel.dataState === "ready" ? (
        <InPageTabs
          ariaLabel="Governance workflow views"
          queryKey="view"
          tabs={tabViews}
        />
      ) : (
        <ShellStatePanel stateModel={stateModel} />
      )}
    </OperatorShellLayout>
  );
}
