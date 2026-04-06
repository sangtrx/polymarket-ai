import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.8 safety rail keeps emergency controls while adding rehearsal evidence visibility", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /pause/i);
  assert.match(rail, /reduce-only/i);
  assert.match(rail, /cancel-all/i);
  assert.match(rail, /executeResumeWorkflow/);
  assert.match(rail, /queryRestoreRehearsals/);
  assert.match(rail, /queryRestoreRehearsalByRunId/);
  assert.match(rail, /Restore rehearsal evidence/);
});

test("Story 3.8 safety rail renders deterministic replay and failed-check diagnostics with one clear action", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /Deterministic replay/);
  assert.match(rail, /recommendedNextAction/);
  assert.match(rail, /integrityChecks/);
  assert.match(rail, /safety-action-recovery-failure/);
});

test("Story 3.8 posture and command-surface wiring propagate rehearsal selectors into resume workflow", () => {
  const posture = read("apps/operator-console/src/lib/risk/posture.ts");
  const commandSurface = read(
    "apps/operator-console/src/components/risk/RiskCommandSurface.tsx",
  );

  assert.match(posture, /resumeArtifactId/);
  assert.match(posture, /resumeIncidentSeverity/);
  assert.match(posture, /resumeRehearsalRunId/);
  assert.match(commandSurface, /resumeArtifactId/);
  assert.match(commandSurface, /resumeIncidentSeverity/);
  assert.match(commandSurface, /resumeRehearsalRunId/);
});

test("Story 3.8 severe resume path requires rehearsal selectors and clears stale rehearsal evidence", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /setLastRestoreRehearsal\(null\);/);
  assert.match(rail, /recovery_rehearsal_selector_required/);
  assert.match(
    rail,
    /Severity-1\/Severity-2 resume requires rehearsal selector metadata/,
  );
});

test("Story 3.8 severe incident failures stay blocked with rehearsal failure diagnostics", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(
    rail,
    /severeIncident && rehearsalEvidence\?\.rehearsalStatus === "failed"/,
  );
  assert.match(rail, /rehearsalFailureDetails/);
  assert.match(rail, /resolveState\("resume"\) === "blocked-with-reasons"/);
  assert.match(rail, /Recovery resume blocked/);
});
