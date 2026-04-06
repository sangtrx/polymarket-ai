import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.3 control-api extends canonical privileged ingress for allocation and rebalance workflows", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/allocation-policies\/\{policy_key\}/);
  assert.match(routes, /\/control\/rebalance\/recommendations\/pending/);
  assert.match(routes, /\/control\/rebalance\/recommendations\/\{recommendation_id\}\/execute/);
  assert.match(routes, /rebalance_recommendation_evaluate/);
  assert.match(routes, /rebalance_recommendation_execute/);
  assert.match(routes, /allocation_policy_service_error_response/);
  assert.match(routes, /authorize_critical_action/);
});

test("Story 3.3 operator-console styling defines progressive-disclosure and evidence surfaces", () => {
  const globals = read("apps/operator-console/src/app/globals.css");

  assert.match(globals, /\.portfolio-allocation-grid/);
  assert.match(globals, /\.allocation-policy-form/);
  assert.match(globals, /\.allocation-advanced-toggle/);
  assert.match(globals, /\.allocation-guidance\[data-tone="warning"\]/);
  assert.match(globals, /\.allocation-guidance\[data-tone="critical"\]/);
  assert.match(globals, /\.allocation-form-error-surface/);
  assert.match(globals, /\.rebalance-pending-list/);
});

test("Story 3.3 recommendation and policy surfaces expose machine-readable evidence and no success-shaped fallback", () => {
  const form = read("apps/operator-console/src/components/portfolio/AllocationPolicyForm.tsx");
  const card = read(
    "apps/operator-console/src/components/portfolio/RebalanceRecommendationCard.tsx",
  );

  assert.match(form, /Policy mutation evidence/);
  assert.match(form, /Error code:/);
  assert.match(form, /role="alert"/);
  assert.match(form, /approval context/i);

  assert.match(card, /Recommendation evidence/);
  assert.match(card, /Pending recommendation queue/);
  assert.match(card, /Error code:/);
  assert.match(card, /Recommended next action:/i);
  assert.match(card, /role="status"/);
});

test("Story 3.3 rebalance card preserves evaluate-load-execute workflow controls and error evidence", () => {
  const card = read(
    "apps/operator-console/src/components/portfolio/RebalanceRecommendationCard.tsx",
  );

  assert.match(card, /Evaluate drift/);
  assert.match(card, /Load pending recommendations/);
  assert.match(card, /Recommendation ID for execution/);
  assert.match(card, /Execute recommendation/);
  assert.match(card, /evaluateRebalanceRecommendation/);
  assert.match(card, /listPendingRebalanceRecommendations/);
  assert.match(card, /executeRebalanceRecommendation/);
  assert.match(card, /Action:\s*\{requestError\.action\}/);
  assert.match(card, /correlation ID:/i);
  assert.match(card, /requestError\.correlationId \?\? "n\/a"/);
});
