import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.5 control-api routes include authenticated incident forensics endpoint", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/incidents\/forensics/);
  assert.match(routes, /read_incident_forensics/);
  assert.match(routes, /incident_query_response|incident_forensics_query_response/);
  assert.match(routes, /incident_service_error_response/);
});

test("Story 3.5 incident timeline component enforces explicit state machine and causal flow framing", () => {
  const timelineCard = read(
    "apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx",
  );

  assert.match(timelineCard, /queryIncidentForensics\(/);
  assert.match(timelineCard, /type ViewState = "loading" \| "ready" \| "empty" \| "error" \| "critical"/);
  assert.match(timelineCard, /Trigger -&gt; Context -&gt; Action -&gt; Verification/);
  assert.match(timelineCard, /Search incidents/);
  assert.match(timelineCard, /Recommended next action:/);
});

test("Story 3.5 timeline query normalizes datetime-local input into UTC timestamps", () => {
  const timelineCard = read(
    "apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx",
  );

  assert.match(timelineCard, /function fromDateTimeLocalValue/);
  assert.match(timelineCard, /new Date\(normalized\)/);
  assert.match(timelineCard, /return parsed\.toISOString\(\)/);
  assert.doesNotMatch(timelineCard, /:00\.000Z/);
});

test("Story 3.5 timeline keeps critical success separate from error surfaces", () => {
  const timelineCard = read(
    "apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx",
  );

  assert.match(timelineCard, /requestError && \(state === "error" \|\| state === "critical"\)/);
  assert.match(timelineCard, /next\.severity === "critical" \|\| next\.severity === "degraded"/);
});

test("Story 3.5 timeline renders effective filter summary for incident searches", () => {
  const timelineCard = read(
    "apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx",
  );

  assert.match(timelineCard, /Effective filters:/);
  assert.match(timelineCard, /market_id=/);
  assert.match(timelineCard, /order_id=/);
  assert.match(timelineCard, /alpha_id=/);
  assert.match(timelineCard, /actor_id=/);
});

test("Story 3.5 incidents route keeps shell composition and persistent safety controls", () => {
  const incidentsPage = read("apps/operator-console/src/app/(incidents)/incidents/page.tsx");
  const layout = read("apps/operator-console/src/components/shell/OperatorShellLayout.tsx");

  assert.match(incidentsPage, /IncidentTimelineCard/);
  assert.match(incidentsPage, /getOperatorConsoleEnv/);
  assert.match(incidentsPage, /baseUrl=\{apiBaseUrl\}/);
  assert.match(layout, /RiskCommandSurface/);
});

test("Story 3.5 styles define incident timeline filters, causal-flow, and timeline evidence states", () => {
  const globalsCss = read("apps/operator-console/src/app/globals.css");

  assert.match(globalsCss, /\.incident-timeline-card/);
  assert.match(globalsCss, /\.incident-metadata-grid/);
  assert.match(globalsCss, /\.incident-filter-grid/);
  assert.match(globalsCss, /\.incident-causal-flow/);
  assert.match(globalsCss, /\.incident-timeline-list/);
  assert.match(globalsCss, /\.incident-error-surface/);
});

test("Story 3.5 dashboard composition remains non-regressive with incident card integration", () => {
  const dashboardPage = read("apps/operator-console/src/app/(dashboard)/dashboard/page.tsx");

  assert.match(dashboardPage, /PortfolioSummaryCard/);
  assert.match(dashboardPage, /ExecutionSummaryCard/);
  assert.match(dashboardPage, /GovernanceQueueCard/);
  assert.match(dashboardPage, /IncidentTimelineCard/);
  assert.match(dashboardPage, /baseUrl=\{apiBaseUrl\}/);
});
