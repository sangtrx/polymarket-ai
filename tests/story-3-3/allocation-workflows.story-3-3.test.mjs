import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.3 dashboard composition embeds allocation policy and rebalance rationale surfaces", () => {
  const dashboard = read("apps/operator-console/src/app/(dashboard)/dashboard/page.tsx");
  const portfolioCard = read(
    "apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx",
  );

  assert.match(dashboard, /PortfolioSummaryCard/);
  assert.match(portfolioCard, /AllocationPolicyForm/);
  assert.match(portfolioCard, /RebalanceRecommendationCard/);
  assert.match(portfolioCard, /portfolio-allocation-grid/);
});

test("Story 3.3 allocation form provides progressive disclosure and blur-validation guidance", () => {
  const form = read(
    "apps/operator-console/src/components/portfolio/AllocationPolicyForm.tsx",
  );

  assert.match(form, /Progressive disclosure/i);
  assert.match(form, /inline\s+validation on blur/i);
  assert.match(form, /Show advanced parameters/);
  assert.match(form, /Hide advanced parameters/);
  assert.match(form, /Risk-impact guidance/i);
  assert.match(form, /Recommended next action:/i);
  assert.match(form, /approval_request_id/);
});

test("Story 3.3 rebalance recommendation card exposes rationale and approval context evidence", () => {
  const card = read(
    "apps/operator-console/src/components/portfolio/RebalanceRecommendationCard.tsx",
  );

  assert.match(card, /Drift rationale \+ approval context/);
  assert.match(card, /Evaluate drift/);
  assert.match(card, /Execute recommendation/);
  assert.match(card, /Pending recommendation queue/);
  assert.match(card, /Recommended next action:/i);
  assert.match(card, /approval context/i);
});
