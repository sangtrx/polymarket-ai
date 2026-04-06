import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.9 safety rail enforces deterministic keyboard confirmation and focus return semantics", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");
  const confirmationHelpers = read(
    "apps/operator-console/src/lib/risk/confirmation-dialog.ts",
  );

  assert.match(rail, /confirmationTriggerRef/);
  assert.match(rail, /restoreFocusAfterConfirmation/);
  assert.match(rail, /window\.addEventListener\("keydown", handleWindowEscape\)/);
  assert.match(rail, /window\.removeEventListener\("keydown", handleWindowEscape\)/);
  assert.match(rail, /aria-modal="true"/);
  assert.match(rail, /onKeyDown=\{handleConfirmationTabLoop\}/);
  assert.match(rail, /const CONFIRMATION_TARGET_MS = 10_000/);
  assert.match(rail, /<= 10s expectation window/);
  assert.match(confirmationHelpers, /resolveConfirmationTabLoop/);
  assert.match(confirmationHelpers, /shouldDismissDangerConfirmation/);
});

test("Story 3.9 banner and incident panels expose explicit assistive announcement summaries with timestamp evidence", () => {
  const banner = read("apps/operator-console/src/components/risk/RiskPostureBanner.tsx");
  const timeline = read(
    "apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx",
  );
  const alerts = read(
    "apps/operator-console/src/components/timeline/IncidentAlertsPanel.tsx",
  );

  assert.match(banner, /announcementSummaryId/);
  assert.match(banner, /Outcome:/);
  assert.match(banner, /Timestamp:/);
  assert.match(timeline, /lastAnnouncementAtUtc/);
  assert.match(timeline, /aria-live="polite"/);
  assert.match(alerts, /lastAnnouncementAtUtc/);
  assert.match(alerts, /aria-live="polite"/);
});

test("Story 3.9 globals define explicit reduced-motion and focus-visible contracts across risk, safety, and incident surfaces", () => {
  const globals = read("apps/operator-console/src/app/globals.css");

  assert.match(globals, /\.safety-action-button:focus-visible/);
  assert.match(globals, /\.incident-filter-input:focus-visible/);
  assert.match(globals, /\.risk-banner--announcement/);
  assert.match(globals, /@media \(prefers-reduced-motion: reduce\)[\s\S]*\.risk-banner/);
  assert.match(
    globals,
    /@media \(prefers-reduced-motion: reduce\)[\s\S]*\.incident-timeline-item[\s\S]*\.incident-alert-item/,
  );
});

test("Story 3.9 operations runbook exists with scope boundaries and cross-runbook links", () => {
  const runbook = read(
    "docs/operations/accessibility-reduced-motion-critical-flows.md",
  );

  assert.match(runbook, /keyboard-only/i);
  assert.match(runbook, /prefers-reduced-motion/i);
  assert.match(runbook, /<= 10s/);
  assert.match(runbook, /non-goals/i);
  assert.match(runbook, /controlled-recovery-readiness-gates\.md/);
  assert.match(runbook, /severity-alert-delivery\.md/);
  assert.match(runbook, /incident-search-causal-timeline-forensics\.md/);
});
