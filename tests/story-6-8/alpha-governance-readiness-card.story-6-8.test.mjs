import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.8 governance card defines explicit loading/ready/empty/error/critical state machine", () => {
  const card = read(
    "apps/operator-console/src/components/governance/AlphaGovernanceReadinessCard.tsx",
  );

  assert.match(
    card,
    /type ViewState = "loading" \| "ready" \| "empty" \| "error" \| "critical"/,
  );
  assert.match(card, /readinessStatus === "blocked"/);
  assert.match(card, /readinessStatus === (?:\\\"ready\\\"|"ready"|'ready')/);
  assert.match(card, /governance-readiness-skeleton/);
  assert.match(card, /governance-readiness-empty-state/);
  assert.match(card, /governance-readiness-error-surface/);
});

test("Story 6.8 governance card preserves metadata-first rendering contract", () => {
  const card = read(
    "apps/operator-console/src/components/governance/AlphaGovernanceReadinessCard.tsx",
  );
  const metadataIndex = card.indexOf("governance-readiness-metadata-grid");
  const detailsIndex = card.indexOf("governance-readiness-details");

  assert.ok(metadataIndex > -1, "metadata grid class should exist");
  assert.ok(detailsIndex > -1, "detail section class should exist");
  assert.ok(
    metadataIndex < detailsIndex,
    "metadata block should appear before detail rendering",
  );
  assert.match(card, /1h/);
  assert.match(card, /24h/);
  assert.match(card, /30d/);
  assert.match(card, /Recommended next action:/);
});

test("Story 6.8 compatibility wrapper preserves GovernanceQueueCard symbol continuity", () => {
  const wrapper = read(
    "apps/operator-console/src/components/governance/GovernanceQueueCard.tsx",
  );

  assert.match(wrapper, /AlphaGovernanceReadinessCard/);
  assert.match(wrapper, /export function GovernanceQueueCard/);
});
