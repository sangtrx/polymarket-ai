import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.6 QA command wires domain, persistence, control-api, and story suites", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-3-6"/);
  assert.match(packageJson, /cargo test -p domain alerts::tests::/);
  assert.match(packageJson, /cargo test -p persistence postgres::incident_alerts::tests::/);
  assert.match(packageJson, /cargo test -p control-api routes::tests::incident_alert_/);
  assert.match(packageJson, /tests\/story-3-6\/\*\.test\.mjs/);
  assert.match(packageJson, /tests\/api\/story-3-6\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-3-6\*\.test\.mjs/);
});

test("Story 3.6 control-api includes authenticated alert retrieval and dispatch routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/incidents\/alerts/);
  assert.match(routes, /list_incident_alerts/);
  assert.match(routes, /dispatch_incident_alert/);
  assert.match(routes, /IncidentAlertsQueryResponse/);
  assert.match(routes, /incident_alert_service_error_response/);
});

test("Story 3.6 incidents and dashboard views integrate alert panel surfaces", () => {
  const incidentsPage = read("apps/operator-console/src/app/(incidents)/incidents/page.tsx");
  const dashboardPage = read("apps/operator-console/src/app/(dashboard)/dashboard/page.tsx");

  assert.match(incidentsPage, /IncidentAlertsPanel/);
  assert.match(incidentsPage, /baseUrl=\{apiBaseUrl\}/);
  assert.match(dashboardPage, /IncidentAlertsPanel/);
  assert.match(dashboardPage, /baseUrl=\{apiBaseUrl\}/);
});

test("Story 3.6 alert panel enforces required guidance fields for warning and critical alerts", () => {
  const panel = read("apps/operator-console/src/components/timeline/IncidentAlertsPanel.tsx");

  assert.match(panel, /queryIncidentAlerts\(/);
  assert.match(panel, /recommendedNextAction/);
  assert.match(panel, /evidenceLink/);
  assert.match(panel, /issuedAt/);
  assert.match(panel, /impactedSubsystem/);
  assert.match(panel, /severity/);
});

test("Story 3.6 global styles include severity alert panel and delivery-attempt evidence surfaces", () => {
  const globalsCss = read("apps/operator-console/src/app/globals.css");

  assert.match(globalsCss, /\.incident-alerts-panel/);
  assert.match(globalsCss, /\.incident-alert-list/);
  assert.match(globalsCss, /\.incident-alert-item/);
  assert.match(globalsCss, /\.incident-alert-attempt-list/);
  assert.match(globalsCss, /\.incident-alert-runbook-link/);
});
