import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

const ROUTE_CASES = [
  {
    key: "dashboard",
    path: "apps/operator-console/src/app/(dashboard)/dashboard/page.tsx",
    source: "dashboard.read-model.shell",
    ariaLabel: "Dashboard workflow views",
  },
  {
    key: "incidents",
    path: "apps/operator-console/src/app/(incidents)/incidents/page.tsx",
    source: "incidents.read-model.shell",
    ariaLabel: "Incident workflow views",
  },
  {
    key: "governance",
    path: "apps/operator-console/src/app/(governance)/governance/page.tsx",
    source: "governance.read-model.shell",
    ariaLabel: "Governance workflow views",
  },
];

test("Story 3.1 e2e route ownership composes shared shell and tab workflow model", () => {
  for (const routeCase of ROUTE_CASES) {
    const route = read(routeCase.path);

    assert.match(route, /resolveShellReadModel/);
    assert.match(route, new RegExp(`"${routeCase.source}"`));
    assert.match(route, /<OperatorShellLayout/);
    assert.match(route, new RegExp(`activeRoute="${routeCase.key}"`));
    assert.match(route, /stateModel\.dataState === "ready"/);
    assert.match(route, /<InPageTabs/);
    assert.match(route, /queryKey="view"/);
    assert.match(route, new RegExp(`ariaLabel="${routeCase.ariaLabel}"`));
    assert.match(route, /<ShellStatePanel stateModel=\{stateModel\} \/>/);
  }
});

test("Story 3.1 e2e navigation manifest maps route groups with page-current semantics", () => {
  const shellLayout = read(
    "apps/operator-console/src/components/shell/OperatorShellLayout.tsx",
  );

  assert.match(
    shellLayout,
    /\{ key: "dashboard", href: "\/dashboard", label: "Dashboard" \}/,
  );
  assert.match(
    shellLayout,
    /\{ key: "incidents", href: "\/incidents", label: "Incidents" \}/,
  );
  assert.match(
    shellLayout,
    /\{ key: "governance", href: "\/governance", label: "Governance" \}/,
  );
  assert.match(
    shellLayout,
    /aria-current=\{route\.key === activeRoute \? "page" : undefined\}/,
  );
  assert.match(shellLayout, /Skip to main content/);
});

test("Story 3.1 e2e fallback surfaces expose explicit timestamped evidence fields", () => {
  const loading = read("apps/operator-console/src/app/loading.tsx");
  const errorBoundary = read("apps/operator-console/src/app/error.tsx");
  const shellStatePanel = read(
    "apps/operator-console/src/components/shell/ShellStatePanel.tsx",
  );

  assert.match(loading, /SHELL_LOADING/);
  assert.match(loading, /Timestamp/);
  assert.match(loading, /role="status"/);

  assert.match(errorBoundary, /SHELL_ROUTE_ERROR/);
  assert.match(errorBoundary, /Error code/);
  assert.match(errorBoundary, /Message/);
  assert.match(errorBoundary, /Timestamp/);
  assert.match(errorBoundary, /role="alert"/);

  assert.match(
    shellStatePanel,
    /role=\{stateModel\.dataState === "loading" \? "status" : "alert"\}/,
  );
  assert.match(shellStatePanel, /Error code/);
  assert.match(shellStatePanel, /Timestamp/);
  assert.match(shellStatePanel, /Source/);
});

test("Story 3.1 e2e top-bar contract preserves freshness and monitor-first policy copy", () => {
  const topBar = read("apps/operator-console/src/components/shell/ShellTopBar.tsx");

  assert.match(topBar, /Freshness evidence/);
  assert.match(topBar, /Source: \{freshness\.source\}/);
  assert.match(topBar, /p95 read-model budget: \{p95TargetMs\}ms/);
  assert.match(topBar, /Mobile policy: monitor-first and action-limited/);
  assert.match(topBar, /Persistent safety action rail is available below/);
});
