import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.9 QA command wires lint, typecheck, build, and story/api/e2e suites", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-3-9"/);
  assert.match(packageJson, /npm run web:lint/);
  assert.match(packageJson, /npm run web:typecheck/);
  assert.match(packageJson, /npm run web:build/);
  assert.match(packageJson, /tests\/story-3-9\/\*\.test\.mjs/);
  assert.match(packageJson, /tests\/api\/story-3-9\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-3-9\*\.test\.mjs/);
});

test("Story 3.9 shell and privileged routes provide a shared assistive announcement region", () => {
  const layout = read("apps/operator-console/src/components/shell/OperatorShellLayout.tsx");
  const dashboard = read("apps/operator-console/src/app/(dashboard)/dashboard/page.tsx");
  const incidents = read("apps/operator-console/src/app/(incidents)/incidents/page.tsx");
  const governance = read("apps/operator-console/src/app/(governance)/governance/page.tsx");

  assert.match(layout, /operator-shell-announcements/);
  assert.match(layout, /aria-live="polite"/);
  assert.match(layout, /aria-atomic="true"/);
  assert.match(dashboard, /riskPosture=\{riskPosture\}/);
  assert.match(incidents, /riskPosture=\{riskPosture\}/);
  assert.match(governance, /riskPosture=\{riskPosture\}/);
});

test("Story 3.9 incident panels provide keyboard-action feedback and non-color semantic state cues", () => {
  const timeline = read(
    "apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx",
  );
  const alerts = read(
    "apps/operator-console/src/components/timeline/IncidentAlertsPanel.tsx",
  );

  assert.match(timeline, /Search incidents/);
  assert.match(timeline, /Refresh timeline/);
  assert.match(timeline, /lastAnnouncementAtUtc/);
  assert.match(timeline, /Status <code>\{state\}<\/code>/);
  assert.match(alerts, /Refresh alerts/);
  assert.match(alerts, /lastAnnouncementAtUtc/);
  assert.match(alerts, /Status <code>\{statusLabel\(state\)\}<\/code>/);
});

test("Story 3.9 safety rail preserves keyboard-only danger confirmation with explicit <=10s announcement evidence", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /timestamped confirmation target \{"<= 10s"\}/);
  assert.match(rail, /role="alertdialog"/);
  assert.match(rail, /aria-modal="true"/);
  assert.match(rail, /onKeyDown=\{handleConfirmationTabLoop\}/);
  assert.match(rail, /Confirm action/);
  assert.match(rail, /Cancel/);
  assert.match(rail, /Outcome:/);
  assert.match(rail, /Timestamp:/);
  assert.match(rail, /Action ID:/);
  assert.match(rail, /Correlation ID:/);
});

test("Story 3.9 reduced-motion and focus-visible contracts cover risk, safety, and incident critical surfaces", () => {
  const globals = read("apps/operator-console/src/app/globals.css");

  assert.match(globals, /@media \(prefers-reduced-motion: reduce\)/);
  assert.match(globals, /\.safety-action-button:focus-visible/);
  assert.match(globals, /\.risk-banner/);
  assert.match(globals, /\.incident-timeline-item/);
  assert.match(globals, /\.incident-alert-item/);
});

test("Story 3.9 runbooks cross-link accessibility operations guidance with incidents, alerts, and recovery", () => {
  const accessibility = read(
    "docs/operations/accessibility-reduced-motion-critical-flows.md",
  );
  const incident = read(
    "docs/operations/incident-search-causal-timeline-forensics.md",
  );
  const alerts = read("docs/operations/severity-alert-delivery.md");
  const recovery = read("docs/operations/controlled-recovery-readiness-gates.md");

  assert.match(accessibility, /incident-search-causal-timeline-forensics\.md/);
  assert.match(accessibility, /severity-alert-delivery\.md/);
  assert.match(accessibility, /controlled-recovery-readiness-gates\.md/);
  assert.match(incident, /accessibility-reduced-motion-critical-flows\.md/);
  assert.match(alerts, /accessibility-reduced-motion-critical-flows\.md/);
  assert.match(recovery, /accessibility-reduced-motion-critical-flows\.md/);
});
