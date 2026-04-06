import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.6 incidents route composes timeline and severity alert panels together", () => {
  const incidentsPage = read("apps/operator-console/src/app/(incidents)/incidents/page.tsx");

  assert.match(incidentsPage, /IncidentTimelineCard/);
  assert.match(incidentsPage, /IncidentAlertsPanel/);
  assert.match(incidentsPage, /baseUrl=\{apiBaseUrl\}/);
});

test("Story 3.6 dashboard route keeps shell composition while adding alert visibility", () => {
  const dashboardPage = read("apps/operator-console/src/app/(dashboard)/dashboard/page.tsx");
  const layout = read("apps/operator-console/src/components/shell/OperatorShellLayout.tsx");

  assert.match(dashboardPage, /PortfolioSummaryCard/);
  assert.match(dashboardPage, /ExecutionSummaryCard/);
  assert.match(dashboardPage, /IncidentTimelineCard/);
  assert.match(dashboardPage, /IncidentAlertsPanel/);
  assert.match(layout, /RiskCommandSurface/);
});

test("Story 3.6 alert panel includes severity, subsystem, issued timestamp, and actionable runbook link", () => {
  const panel = read("apps/operator-console/src/components/timeline/IncidentAlertsPanel.tsx");

  assert.match(panel, /severity/i);
  assert.match(panel, /impacted subsystem/i);
  assert.match(panel, /issued/i);
  assert.match(panel, /Recommended next action:/i);
  assert.match(panel, /Runbook evidence/);
});

test("Story 3.6 alert panel surfaces auditable fallback delivery attempts for operator triage", () => {
  const panel = read("apps/operator-console/src/components/timeline/IncidentAlertsPanel.tsx");

  assert.match(panel, /incident-alert-attempt-list/);
  assert.match(panel, /Attempt \{attempt\.attemptNumber\}/);
  assert.match(panel, /channel <code>\{attempt\.channel\}<\/code>/);
  assert.match(panel, /outcome <code>\{attempt\.outcome\}<\/code>/);
  assert.match(panel, /attempted \{attempt\.attemptedAt\}/);
  assert.match(panel, /failed \$\{attempt\.failedAt\}/);
});

test("Story 3.6 alert panel preserves accessible status and failure semantics", () => {
  const panel = read("apps/operator-console/src/components/timeline/IncidentAlertsPanel.tsx");

  assert.match(panel, /aria-label="Severity alerts with recommended operator actions"/);
  assert.match(panel, /role="status"/);
  assert.match(panel, /role="alert"/);
  assert.match(panel, /Refresh alerts/);
});

test("Story 3.6 operations runbooks cross-link incident forensics, alert delivery, and emergency controls", () => {
  const alertsRunbook = read("docs/operations/severity-alert-delivery.md");
  const incidentRunbook = read("docs/operations/incident-search-causal-timeline-forensics.md");
  const emergencyRunbook = read("docs/operations/emergency-safe-state-controls.md");

  assert.match(alertsRunbook, /incident-search-causal-timeline-forensics\.md/);
  assert.match(alertsRunbook, /emergency-safe-state-controls\.md/);
  assert.match(incidentRunbook, /severity-alert-delivery\.md/);
  assert.match(emergencyRunbook, /severity-alert-delivery\.md/);
});
