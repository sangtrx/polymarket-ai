import { InPageTabs } from "@/components/shell/InPageTabs";
import { OperatorShellLayout } from "@/components/shell/OperatorShellLayout";
import { ShellStatePanel } from "@/components/shell/ShellStatePanel";
import { IncidentAlertsPanel } from "@/components/timeline/IncidentAlertsPanel";
import { IncidentTimelineCard } from "@/components/timeline/IncidentTimelineCard";
import { getOperatorConsoleEnv } from "@/lib/env";
import { resolveRiskPostureViewModel } from "@/lib/risk/posture";
import {
  type ShellSearchParams,
  resolveShellReadModel,
} from "@/lib/shell/read-models";

interface IncidentsPageProps {
  searchParams?: Promise<ShellSearchParams>;
}

export default async function IncidentsPage({ searchParams }: IncidentsPageProps) {
  const resolvedSearchParams = (await searchParams) ?? {};
  const stateModel = resolveShellReadModel(
    resolvedSearchParams,
    "incidents.read-model.shell",
  );
  const { apiBaseUrl } = getOperatorConsoleEnv();
  const riskPosture = resolveRiskPostureViewModel(resolvedSearchParams, stateModel);

  const tabViews = [
    {
      id: "timeline",
      label: "Timeline",
      content: (
        <section className="shell-panel-grid">
          <IncidentTimelineCard
            baseUrl={apiBaseUrl}
            freshness={stateModel.freshness}
          />
          <IncidentAlertsPanel
            baseUrl={apiBaseUrl}
            freshness={stateModel.freshness}
          />
          <article className="shell-panel shell-panel-grid-item">
            <p className="type-eyebrow">Incident context</p>
            <h2 className="type-heading-m">Forensics workflow summary</h2>
            <p className="type-body text-muted">
              Route keeps single-submit incident search aligned to Trigger -&gt;
              Context -&gt; Action -&gt; Verification framing while preserving
              persistent safety-rail controls for pause/reduce-only/cancel-all
              interventions.
            </p>
            <p className="type-metadata text-muted">
              p95 incident-query target remains 5,000ms for investigative
              workflows.
            </p>
          </article>
        </section>
      ),
    },
    {
      id: "triage",
      label: "Triage",
      content: (
        <article className="shell-panel">
          <p className="type-eyebrow">Triage flow</p>
            <h2 className="type-heading-m">Operator guidance</h2>
            <p className="type-body text-muted">
              Use this route to validate timeline evidence, then escalate through
              governance when root cause and recommended action are clear.
              Mutation requests continue through explicit safety-rail confirmation
              with machine-readable outcome evidence.
            </p>
          </article>
        ),
      },
  ] as const;

  return (
    <OperatorShellLayout
      activeRoute="incidents"
      description="Incident search and causal timeline forensics with explicit failure evidence."
      freshness={stateModel.freshness}
      p95TargetMs={stateModel.p95TargetMs}
      riskPosture={riskPosture}
      title="Incident response workspace"
    >
      {stateModel.dataState === "ready" ? (
        <InPageTabs
          ariaLabel="Incident workflow views"
          queryKey="view"
          tabs={tabViews}
        />
      ) : (
        <ShellStatePanel stateModel={stateModel} />
      )}
    </OperatorShellLayout>
  );
}
