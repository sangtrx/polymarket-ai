import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.8 route composition keeps GovernanceQueueCard symbol while wiring baseUrl + freshness in dashboard and governance routes", () => {
  const dashboard = read("apps/operator-console/src/app/(dashboard)/dashboard/page.tsx");
  const governance = read("apps/operator-console/src/app/(governance)/governance/page.tsx");

  assert.match(dashboard, /GovernanceQueueCard/);
  assert.match(dashboard, /baseUrl=\{apiBaseUrl\}/);
  assert.match(dashboard, /freshness=\{stateModel\.freshness\}/);

  assert.match(governance, /GovernanceQueueCard/);
  assert.match(governance, /baseUrl=\{apiBaseUrl\}/);
  assert.match(governance, /freshness=\{stateModel\.freshness\}/);
});

test("Story 6.8 governance card styles define metadata, skeleton, empty, blocked, and error contracts", () => {
  const globalsCss = read("apps/operator-console/src/app/globals.css");

  assert.match(globalsCss, /\.governance-readiness-card/);
  assert.match(globalsCss, /\.governance-readiness-metadata-grid/);
  assert.match(globalsCss, /\.governance-readiness-skeleton/);
  assert.match(globalsCss, /\.governance-readiness-empty-state/);
  assert.match(globalsCss, /\.governance-readiness-blocked-list/);
  assert.match(globalsCss, /\.governance-readiness-error-surface/);
  assert.match(globalsCss, /\.governance-readiness-announcement/);
});

test("Story 6.8 QA command wiring executes web quality gates with story/api/e2e suites", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-6-8"/);
  assert.match(packageJson, /npm run web:lint/);
  assert.match(packageJson, /npm run web:typecheck/);
  assert.match(packageJson, /npm run web:build/);
  assert.match(packageJson, /tests\/story-6-8\/\*\.test\.mjs/);
  assert.match(packageJson, /tests\/api\/story-6-8\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-6-8\*\.test\.mjs/);
});

test("Story 6.8 runbook continuity links governance readiness with Story 6.5/6.6/6.7 operations", () => {
  const runbook = read("docs/operations/alpha-governance-readiness-card.md");

  assert.match(runbook, /promotion-decisions/);
  assert.match(runbook, /shadow-evaluations/);
  assert.match(runbook, /validation-runs/);
  assert.match(runbook, /alpha-health-metrics/);
  assert.match(runbook, /alpha-threshold-breaches/);
  assert.match(runbook, /alpha-promotion-lifecycle-governance\.md/);
  assert.match(runbook, /alpha-counterfactual-replay-stress-gating\.md/);
  assert.match(runbook, /alpha-live-health-monitoring-threshold-breaches\.md/);
});

test("Story 6.8 card exposes machine-readable blocked and fail-closed accessibility surfaces", () => {
  const card = read(
    "apps/operator-console/src/components/governance/AlphaGovernanceReadinessCard.tsx",
  );

  assert.match(card, /governance-readiness-blocked-list/);
  assert.match(card, /Recommended next action:/);
  assert.match(card, /role="status"/);
  assert.match(card, /aria-live="polite"/);
  assert.match(card, /role="alert"/);
  assert.match(card, /Error code:/);
  assert.match(card, /endpoint:/);
  assert.match(card, /requestError\.correlationId \?\? "n\/a"/);
});
