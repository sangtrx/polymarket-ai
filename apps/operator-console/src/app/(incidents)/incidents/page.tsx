import { InPageTabs } from "@/components/shell/InPageTabs";
import { OperatorShellLayout } from "@/components/shell/OperatorShellLayout";
import { ShellStatePanel } from "@/components/shell/ShellStatePanel";
import { IncidentTimelineCard } from "@/components/timeline/IncidentTimelineCard";
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
  const riskPosture = resolveRiskPostureViewModel(resolvedSearchParams, stateModel);

  const tabViews = [
    {
      id: "timeline",
      label: "Timeline",
      content: (
        <section className="shell-panel-grid">
          <IncidentTimelineCard freshness={stateModel.freshness} />
          <article className="shell-panel shell-panel-grid-item">
            <p className="type-eyebrow">Incident context</p>
            <h2 className="type-heading-m">Root-cause queue</h2>
            <p className="type-body text-muted">
              Route remains traceability-first while incident data is
              investigated. Persistent safety controls are available in the rail
              for pause/reduce-only/cancel-all interventions.
            </p>
            <p className="type-metadata text-muted">
              Monitor-first policy applies on mobile breakpoints.
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
              Use this shell route to inspect incident freshness and timeline
              ordering before any governance escalation. Mutation requests route
              through explicit safety-rail confirmation and machine-readable
              outcome evidence.
            </p>
          </article>
        ),
      },
  ] as const;

  return (
    <OperatorShellLayout
      activeRoute="incidents"
      description="Timeline and triage shell surfaces with explicit failure evidence."
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
