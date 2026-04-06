import Link from "next/link";
import { RiskCommandSurface } from "@/components/risk/RiskCommandSurface";
import { ShellTopBar } from "@/components/shell/ShellTopBar";
import type { RiskPostureViewModel } from "@/lib/risk/posture";
import type { ShellFreshnessSnapshot } from "@/lib/shell/read-models";

const PRIMARY_ROUTES = [
  { key: "dashboard", href: "/dashboard", label: "Dashboard" },
  { key: "incidents", href: "/incidents", label: "Incidents" },
  { key: "governance", href: "/governance", label: "Governance" },
] as const;

type RouteKey = (typeof PRIMARY_ROUTES)[number]["key"];

interface OperatorShellLayoutProps {
  activeRoute: RouteKey;
  title: string;
  description: string;
  freshness: ShellFreshnessSnapshot;
  p95TargetMs: number;
  riskPosture: RiskPostureViewModel;
  children: React.ReactNode;
}

export function OperatorShellLayout({
  activeRoute,
  title,
  description,
  freshness,
  p95TargetMs,
  riskPosture,
  children,
}: OperatorShellLayoutProps) {
  const activeRouteLabel =
    PRIMARY_ROUTES.find((route) => route.key === activeRoute)?.label ?? activeRoute;

  return (
    <div className="shell-root">
      <a className="skip-link" href="#main-content">
        Skip to main content
      </a>

      <header className="shell-header-stack">
        <ShellTopBar
          description={description}
          freshness={freshness}
          p95TargetMs={p95TargetMs}
          title={title}
        />
        <RiskCommandSurface riskPosture={riskPosture} />
      </header>

      <div
        id="operator-shell-announcements"
        className="assistive-announcement operator-shell-announcements"
        aria-live="polite"
        aria-atomic="true"
      >
        Active route: {activeRouteLabel}. Risk posture: {riskPosture.posture}. Last evidence
        timestamp: {riskPosture.evidence.lastUpdatedIso}.
      </div>

      <div className="shell-frame">
        <nav aria-label="Primary navigation" className="shell-rail">
          <ul className="shell-rail-list">
            {PRIMARY_ROUTES.map((route) => (
              <li key={route.key}>
                <Link
                  aria-current={route.key === activeRoute ? "page" : undefined}
                  className="shell-rail-link"
                  data-active={route.key === activeRoute ? "true" : "false"}
                  href={route.href}
                >
                  {route.label}
                </Link>
              </li>
            ))}
          </ul>
        </nav>

        <main className="shell-content" id="main-content">
          {children}
        </main>
      </div>
    </div>
  );
}
