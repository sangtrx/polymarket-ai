import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.2 composes persistent risk command surface across privileged routes", () => {
  const layout = read("apps/operator-console/src/components/shell/OperatorShellLayout.tsx");
  const dashboard = read("apps/operator-console/src/app/(dashboard)/dashboard/page.tsx");
  const incidents = read("apps/operator-console/src/app/(incidents)/incidents/page.tsx");
  const governance = read("apps/operator-console/src/app/(governance)/governance/page.tsx");
  const commandSurface = read(
    "apps/operator-console/src/components/risk/RiskCommandSurface.tsx",
  );

  assert.match(layout, /RiskCommandSurface/);
  assert.match(layout, /riskPosture=\{riskPosture\}/);
  assert.match(commandSurface, /useEffect/);
  assert.match(commandSurface, /setActivePosture\(riskPosture\)/);

  for (const route of [dashboard, incidents, governance]) {
    assert.match(route, /resolveRiskPostureViewModel/);
    assert.match(route, /riskPosture=\{riskPosture\}/);
  }
});

test("Story 3.2 banner provides stateful accessibility and evidence semantics", () => {
  const banner = read(
    "apps/operator-console/src/components/risk/RiskPostureBanner.tsx",
  );

  assert.match(banner, /role="status"/);
  assert.match(banner, /aria-live/);
  assert.match(banner, /critical/);
  assert.match(banner, /locked-safe/);
  assert.match(banner, /recommended next action/i);
  assert.match(banner, /actor\/source/i);
  assert.match(banner, /action id/i);
  assert.match(banner, /correlation id/i);
  assert.match(banner, /audit reference/i);
});

test("Story 3.2 action rail preserves explicit confirmation and gating contracts", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /pause/i);
  assert.match(rail, /reduce-only/i);
  assert.match(rail, /cancel-all/i);
  assert.match(rail, /resume/i);
  assert.match(rail, /<=\s*2 interactions/i);
  assert.match(rail, /<=\s*1s/i);
  assert.match(rail, /<=\s*5s/i);
  assert.match(rail, /<=\s*10s/i);
  assert.match(rail, /story 3\.7/i);
  assert.match(rail, /confirm/i);
  assert.match(rail, /actionInFlightRef/);
  assert.match(rail, /waitForConfirmedActionResult/);
  assert.match(rail, /emergency_control_latency_threshold_exceeded/);
  assert.match(rail, /aria-labelledby/);
  assert.match(rail, /aria-describedby/);
  assert.match(rail, /error code/i);
});

test("Story 3.2 styles are token-driven and define risk/action semantic surfaces", () => {
  const tokens = read("apps/operator-console/src/styles/tokens.css");
  const globals = read("apps/operator-console/src/app/globals.css");

  assert.match(tokens, /--risk-banner-normal-bg/i);
  assert.match(tokens, /--risk-banner-warning-bg/i);
  assert.match(tokens, /--risk-banner-critical-bg/i);
  assert.match(tokens, /--risk-banner-locked-safe-bg/i);
  assert.match(tokens, /--safety-action-danger-bg/i);
  assert.match(tokens, /--safety-action-secondary-bg/i);
  assert.match(tokens, /--safety-action-gated-bg/i);
  assert.match(tokens, /--safety-action-completed-bg/i);

  assert.match(globals, /\.risk-banner/i);
  assert.match(globals, /\.safety-action-rail/i);
  assert.match(globals, /\.safety-action-warning/i);
  assert.match(globals, /\.safety-action-button/i);
});
