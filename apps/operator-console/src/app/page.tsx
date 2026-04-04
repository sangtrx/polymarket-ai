import Link from "next/link";
import { ExecutionSummaryCard } from "@/components/execution/ExecutionSummaryCard";
import { GovernanceQueueCard } from "@/components/governance/GovernanceQueueCard";
import { RiskPostureCard } from "@/components/risk/RiskPostureCard";
import { IncidentTimelineCard } from "@/components/timeline/IncidentTimelineCard";
import { getOperatorConsoleEnv } from "@/lib/env";

export default function Home() {
  const env = getOperatorConsoleEnv();

  return (
    <main className="mx-auto flex w-full max-w-6xl flex-1 flex-col gap-6 px-6 py-8">
      <header className="card">
        <p className="eyebrow">Story 1.1 Bootstrap Baseline</p>
        <h1 className="text-3xl font-semibold">Operator Console Shell</h1>
        <p className="muted mt-2">
          Control API base URL: <code>{env.apiBaseUrl}</code>
        </p>
        <nav className="mt-4 flex flex-wrap gap-3">
          <Link className="badge" href="/dashboard">
            Dashboard
          </Link>
          <Link className="badge" href="/incidents">
            Incidents
          </Link>
          <Link className="badge" href="/governance">
            Governance
          </Link>
        </nav>
      </header>

      <section className="grid gap-4 md:grid-cols-2">
        <RiskPostureCard />
        <ExecutionSummaryCard />
        <GovernanceQueueCard />
        <IncidentTimelineCard />
      </section>
    </main>
  );
}
