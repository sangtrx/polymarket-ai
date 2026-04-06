import { InPageTabs } from "@/components/shell/InPageTabs";
import { OperatorShellLayout } from "@/components/shell/OperatorShellLayout";
import { ShellStatePanel } from "@/components/shell/ShellStatePanel";
import { IncidentTimelineCard } from "@/components/timeline/IncidentTimelineCard";
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
              Route remains read-only while incident data is investigated.
              Destructive controls remain disabled until Story 3.2.
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
            ordering before any governance escalation. No mutation controls are
            available in Story 3.1.
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
