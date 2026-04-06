import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.5 QA command wires Rust and web incident-forensics suites", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-3-5"/);
  assert.match(packageJson, /cargo test -p domain incidents::tests::/);
  assert.match(packageJson, /cargo test -p persistence postgres::incident_query_views::tests::/);
  assert.match(packageJson, /cargo test -p control-api routes::tests::incident_forensics_/);
  assert.match(packageJson, /tests\/story-3-5\/\*\.test\.mjs/);
  assert.match(packageJson, /tests\/api\/story-3-5\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-3-5\*\.test\.mjs/);
});

test("Story 3.5 incidents route keeps shell-state fallback and timeline tab composition", () => {
  const incidentsPage = read("apps/operator-console/src/app/(incidents)/incidents/page.tsx");

  assert.match(incidentsPage, /InPageTabs/);
  assert.match(incidentsPage, /ShellStatePanel/);
  assert.match(incidentsPage, /IncidentTimelineCard/);
  assert.match(incidentsPage, /activeRoute="incidents"/);
  assert.match(incidentsPage, /single-submit incident search aligned to Trigger/);
});

test("Story 3.5 timeline component exposes loading, empty, error, critical, and ready contracts", () => {
  const timelineCard = read(
    "apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx",
  );

  assert.match(timelineCard, /type ViewState = "loading" \| "ready" \| "empty" \| "error" \| "critical"/);
  assert.match(timelineCard, /state === "loading"/);
  assert.match(timelineCard, /state === "empty"/);
  assert.match(timelineCard, /state === "error" \|\| state === "critical"/);
  assert.match(timelineCard, /result && result.events.length > 0/);
  assert.match(timelineCard, /incident-causal-flow/);
  assert.match(timelineCard, /incident-timeline-list/);
});

test("Story 3.5 control-api response contract includes latency SLO and causal-flow evidence", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /p95_latency_target_ms:\s*5_000/);
  assert.match(routes, /query_latency_ms/);
  assert.match(routes, /IncidentCausalFlowSummary/);
  assert.match(routes, /IncidentForensicsQueryResponse/);
  assert.match(routes, /incident_service_error_status/);
});

test("Story 3.5 timeline form keeps canonical filters with UTC-bounded search controls", () => {
  const timelineCard = read(
    "apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx",
  );
  const forensicsClient = read("apps/operator-console/src/lib/incidents/forensics.ts");

  assert.match(timelineCard, /Market ID/);
  assert.match(timelineCard, /Order ID/);
  assert.match(timelineCard, /Alpha ID/);
  assert.match(timelineCard, /Actor ID/);
  assert.match(timelineCard, /Start timestamp \(UTC\)/);
  assert.match(timelineCard, /End timestamp \(UTC\)/);
  assert.match(timelineCard, /Refresh timeline/);
  assert.match(forensicsClient, /params\.set\("alpha_id", normalizedAlphaId\)/);
  assert.match(forensicsClient, /params\.set\("actor_id", normalizedActorId\)/);
});
