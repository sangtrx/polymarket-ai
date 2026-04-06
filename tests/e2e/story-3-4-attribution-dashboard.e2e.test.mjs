import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.4 dashboard composition keeps existing control surfaces while adding attribution summary", () => {
  const dashboardPage = read("apps/operator-console/src/app/(dashboard)/dashboard/page.tsx");
  const portfolioCard = read(
    "apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx",
  );

  assert.match(dashboardPage, /PortfolioSummaryCard/);
  assert.match(dashboardPage, /ExecutionSummaryCard/);
  assert.match(dashboardPage, /GovernanceQueueCard/);

  assert.match(portfolioCard, /PnlAttributionSummaryCard/);
  assert.match(portfolioCard, /AllocationPolicyForm/);
  assert.match(portfolioCard, /RebalanceRecommendationCard/);
});

test("Story 3.4 attribution styles define skeleton, metadata, empty, error, and table contracts", () => {
  const globalsCss = read("apps/operator-console/src/app/globals.css");

  assert.match(globalsCss, /\.attribution-summary-card/);
  assert.match(globalsCss, /\.attribution-metadata-grid/);
  assert.match(globalsCss, /\.attribution-skeleton/);
  assert.match(globalsCss, /\.attribution-empty-state/);
  assert.match(globalsCss, /\.attribution-error-surface/);
  assert.match(globalsCss, /\.attribution-table/);
});

test("Story 3.4 summary component includes explicit UI states and non-blocking refresh action", () => {
  const summaryCard = read(
    "apps/operator-console/src/components/portfolio/PnlAttributionSummaryCard.tsx",
  );

  assert.match(summaryCard, /queryCostAwareAttribution\(/);
  assert.match(summaryCard, /state === "loading"/);
  assert.match(summaryCard, /state === "empty"/);
  assert.match(summaryCard, /state === "error" \|\| state === "critical"/);
  assert.match(summaryCard, /setRefreshTick\(\(value\) => value \+ 1\)/);
  assert.match(summaryCard, /aria-busy=\{state === "loading"\}/);
});

test("Story 3.4 summary component pins canonical periods and inclusive/exclusive boundary labels", () => {
  const summaryCard = read(
    "apps/operator-console/src/components/portfolio/PnlAttributionSummaryCard.tsx",
  );

  assert.match(summaryCard, /const PERIOD_OPTIONS: AttributionPeriod\[] = \["1h", "24h", "30d"\]/);
  assert.match(summaryCard, /Window start \(inclusive\)/);
  assert.match(summaryCard, /Window end \(exclusive\)/);
});

test("Story 3.4 summary component escalates dependency failures to critical guidance", () => {
  const summaryCard = read(
    "apps/operator-console/src/components/portfolio/PnlAttributionSummaryCard.tsx",
  );

  assert.match(summaryCard, /error\.errorCode\.includes\("projection_unavailable"\)/);
  assert.match(summaryCard, /error\.errorCode\.includes\("stale_source"\)/);
  assert.match(summaryCard, /error\.errorCode\.includes\("persistence_unavailable"\)/);
  assert.match(
    summaryCard,
    /Validate reconciliation freshness and projection health before acting on this view\./,
  );
  assert.match(summaryCard, /Error code:/);
  assert.match(summaryCard, /correlation ID:/);
});
