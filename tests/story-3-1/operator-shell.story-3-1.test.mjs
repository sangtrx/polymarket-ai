import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.1 token architecture defines layered semantic tokens", () => {
  const tokens = read("apps/operator-console/src/styles/tokens.css");

  assert.match(tokens, /Foundation token layer/i);
  assert.match(tokens, /Domain semantic token layer/i);
  assert.match(tokens, /Product shell token layer/i);
  assert.match(tokens, /--risk-normal/i);
  assert.match(tokens, /--risk-warning/i);
  assert.match(tokens, /--risk-critical/i);
  assert.match(tokens, /--risk-locked-safe/i);
  assert.match(tokens, /--layout-grid-columns-mobile:\s*4/i);
  assert.match(tokens, /--layout-grid-columns-tablet:\s*8/i);
  assert.match(tokens, /--layout-grid-columns-desktop:\s*12/i);
});

test("Story 3.1 removes direct hex usage from route and component surfaces", () => {
  const hexPattern = /#(?:[0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})\b/i;
  const files = [
    "apps/operator-console/src/app/globals.css",
    "apps/operator-console/src/app/page.tsx",
    "apps/operator-console/src/app/(dashboard)/dashboard/page.tsx",
    "apps/operator-console/src/app/(incidents)/incidents/page.tsx",
    "apps/operator-console/src/app/(governance)/governance/page.tsx",
    "apps/operator-console/src/components/risk/RiskPostureCard.tsx",
    "apps/operator-console/src/components/execution/ExecutionSummaryCard.tsx",
    "apps/operator-console/src/components/governance/GovernanceQueueCard.tsx",
    "apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx",
    "apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx",
  ];

  for (const file of files) {
    const content = read(file);
    assert.ok(!hexPattern.test(content), `${file} contains direct hex colors`);
  }
});

test("Story 3.1 shell layout uses required landmarks and primary navigation", () => {
  const shellLayout = read(
    "apps/operator-console/src/components/shell/OperatorShellLayout.tsx",
  );

  assert.match(shellLayout, /<header[\s>]/i);
  assert.match(shellLayout, /<nav[\s>]/i);
  assert.match(shellLayout, /<main[\s>]/i);
  assert.match(shellLayout, /aria-label="Primary navigation"/);
  assert.match(shellLayout, /aria-current=\{route\.key === activeRoute \? "page" : undefined\}/);
  assert.match(shellLayout, /skip to main content/i);
});

test("Story 3.1 in-page tabs implement URL-aware accessible keyboard model", () => {
  const tabs = read("apps/operator-console/src/components/shell/InPageTabs.tsx");

  assert.match(tabs, /role="tablist"/);
  assert.match(tabs, /role="tab"/);
  assert.match(tabs, /aria-controls=/);
  assert.match(tabs, /aria-selected=/);
  assert.match(tabs, /ArrowRight/);
  assert.match(tabs, /ArrowLeft/);
  assert.match(tabs, /Home/);
  assert.match(tabs, /End/);
  assert.match(tabs, /normalizeTabs/);
  assert.match(tabs, /useEffect/);
  assert.match(tabs, /const fallbackId = resolvedTabs\[0\]\?\.id;/);
  assert.match(tabs, /resolvedTabs\.some\(\(tab\) => tab\.id === current\)/);
  assert.match(tabs, /router\.replace/);
});

test("Story 3.1 route shell pages preserve ownership with deterministic shell composition", () => {
  const dashboard = read(
    "apps/operator-console/src/app/(dashboard)/dashboard/page.tsx",
  );
  const incidents = read(
    "apps/operator-console/src/app/(incidents)/incidents/page.tsx",
  );
  const governance = read(
    "apps/operator-console/src/app/(governance)/governance/page.tsx",
  );

  assert.match(dashboard, /OperatorShellLayout/);
  assert.match(incidents, /OperatorShellLayout/);
  assert.match(governance, /OperatorShellLayout/);
  assert.match(dashboard, /PortfolioSummaryCard/);
  assert.match(dashboard, /InPageTabs/);
  assert.match(incidents, /InPageTabs/);
  assert.match(governance, /InPageTabs/);
});

test("Story 3.1 responsive policy enforces deterministic breakpoints and monitor-first controls", () => {
  const globals = read("apps/operator-console/src/app/globals.css");
  const topBar = read("apps/operator-console/src/components/shell/ShellTopBar.tsx");

  assert.match(globals, /layout-grid-columns-mobile\), minmax\(0, 1fr\)\)/i);
  assert.match(globals, /layout-grid-columns-tablet\), minmax\(0, 1fr\)\)/i);
  assert.match(globals, /layout-grid-columns-desktop\), minmax\(0, 1fr\)\)/i);
  assert.match(topBar, /monitor-first/i);
  assert.match(topBar, /persistent safety action rail/i);
});

test("Story 3.1 accessibility baseline includes focus and reduced-motion safeguards", () => {
  const globals = read("apps/operator-console/src/app/globals.css");

  assert.match(globals, /:focus-visible/);
  assert.match(globals, /prefers-reduced-motion:\s*reduce/i);
});

test("Story 3.1 shell state surfaces include explicit error evidence and freshness metadata", () => {
  const shellState = read("apps/operator-console/src/components/shell/ShellStatePanel.tsx");
  const shellModels = read("apps/operator-console/src/lib/shell/read-models.ts");

  assert.match(shellState, /error code/i);
  assert.match(shellState, /timestamp/i);
  assert.match(shellModels, /lastUpdatedIso/);
  assert.match(shellModels, /isStale/);
  assert.match(shellModels, /source/);
  assert.match(shellModels, /unauthorized/i);
  assert.match(shellModels, /function resolveSource/);
  assert.match(shellModels, /source = resolveSource/);
  assert.match(shellModels, /return "error";/);
});

test("Story 3.1 route-level fallback pages preserve explicit evidence contract", () => {
  const loading = read("apps/operator-console/src/app/loading.tsx");
  const errorBoundary = read("apps/operator-console/src/app/error.tsx");

  assert.match(loading, /SHELL_LOADING/);
  assert.match(loading, /timestamp/i);
  assert.match(loading, /role="status"/);
  assert.match(loading, /shell-state-panel--full/);

  assert.match(errorBoundary, /SHELL_ROUTE_ERROR/);
  assert.match(errorBoundary, /error code/i);
  assert.match(errorBoundary, /message/i);
  assert.match(errorBoundary, /timestamp/i);
  assert.match(errorBoundary, /role="alert"/);
  assert.match(errorBoundary, /shell-state-panel--full/);
  assert.match(errorBoundary, /process\.env\.NODE_ENV === "production"/);
});
