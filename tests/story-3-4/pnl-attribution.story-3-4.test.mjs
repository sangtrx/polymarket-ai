import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.4 portfolio summary composes cost-aware attribution with existing 3.3 allocation workflows", () => {
  const portfolioCard = read(
    "apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx",
  );

  assert.match(portfolioCard, /PnlAttributionSummaryCard/);
  assert.match(portfolioCard, /AllocationPolicyForm/);
  assert.match(portfolioCard, /RebalanceRecommendationCard/);
  assert.match(portfolioCard, /Cost-aware realized\/unrealized attribution/i);
});

test("Story 3.4 attribution summary card defines explicit loading/ready/empty/error/critical state machine", () => {
  const summaryCard = read(
    "apps/operator-console/src/components/portfolio/PnlAttributionSummaryCard.tsx",
  );

  assert.match(summaryCard, /type ViewState = AttributionDataState \| "loading" \| "error" \| "critical"/);
  assert.match(summaryCard, /setState\("loading"\)/);
  assert.match(summaryCard, /setState\(next\.dataState\)/);
  assert.match(summaryCard, /setState\(isCriticalError\(clientError\) \? "critical" : "error"\)/);
  assert.match(summaryCard, /Recommended next action:/);
  assert.match(summaryCard, /attribution-skeleton/);
  assert.match(summaryCard, /attribution-empty-state/);
  assert.match(summaryCard, /attribution-error-surface/);
});

test("Story 3.4 metadata-first contract renders metadata before narrative detail", () => {
  const summaryCard = read(
    "apps/operator-console/src/components/portfolio/PnlAttributionSummaryCard.tsx",
  );
  const metadataIndex = summaryCard.indexOf("attribution-metadata-grid");
  const narrativeIndex = summaryCard.indexOf("attribution-narrative");

  assert.ok(metadataIndex > -1, "metadata grid class should exist");
  assert.ok(narrativeIndex > -1, "narrative class should exist");
  assert.ok(
    metadataIndex < narrativeIndex,
    "metadata block should appear before narrative copy",
  );
});

test("Story 3.4 breakdown table exposes cost decomposition and machine-trace fields", () => {
  const table = read(
    "apps/operator-console/src/components/portfolio/PnlAttributionBreakdownTable.tsx",
  );

  assert.match(table, /Cost-aware attribution rows sorted/i);
  assert.match(table, /Realized/);
  assert.match(table, /Unrealized/);
  assert.match(table, /Fees/);
  assert.match(table, /Rebates/);
  assert.match(table, /Incentives/);
  assert.match(table, /Net cost/);
  assert.match(table, /Reason/);
  assert.match(table, /Correlation/);
  assert.match(table, /Snapshot/);
  assert.match(table, /Run/);
});
