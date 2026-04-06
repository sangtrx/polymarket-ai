import Link from "next/link";
import { ShellTopBar } from "@/components/shell/ShellTopBar";
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
  children: React.ReactNode;
}

export function OperatorShellLayout({
  activeRoute,
  title,
  description,
  freshness,
  p95TargetMs,
  children,
}: OperatorShellLayoutProps) {
  return (
    <div className="shell-root">
      <a className="skip-link" href="#main-content">
        Skip to main content
      </a>

      <header>
        <ShellTopBar
          description={description}
          freshness={freshness}
          p95TargetMs={p95TargetMs}
          title={title}
        />
      </header>

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
