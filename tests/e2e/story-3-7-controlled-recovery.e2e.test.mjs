import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.7 safety rail keeps Story 3.2 emergency controls while adding recovery workflow controls", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /pause/i);
  assert.match(rail, /reduce-only/i);
  assert.match(rail, /cancel-all/i);
  assert.match(rail, /waitForConfirmedActionResult/);
  assert.match(rail, /invokeRecoveryReadinessEvaluation/);
  assert.match(rail, /invokeRecoveryResume/);
  assert.match(rail, /executeResumeWorkflow/);
  assert.match(rail, /resumeState: "gated" \| "in-progress" \| "completed" \| "blocked-with-reasons"/);
});

test("Story 3.7 safety rail renders blocked gate diagnostics in Trigger -> Context -> Action -> Verification order", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /Recovery resume blocked/);
  assert.match(rail, /blocked-with-reasons/);
  assert.match(rail, /Trigger:/);
  assert.match(rail, /Context:/);
  assert.match(rail, /Action:/);
  assert.match(rail, /Verification:/);
  assert.match(rail, /effectiveResumeFailures/);
});

test("Story 3.7 safety rail renders timestamped controlled recovery verification evidence", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /Controlled recovery verification/);
  assert.match(rail, /Run ID/);
  assert.match(rail, /Verification timestamp/);
  assert.match(rail, /verificationReasonCode/);
  assert.match(rail, /resumedAtUtc/);
});

test("Story 3.7 safety rail preserves accessible recovery status and escalation semantics", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /aria-live="assertive"/);
  assert.match(rail, /className="safety-action-recovery-blocked" role="status"/);
  assert.match(rail, /className="safety-action-recovery-evidence" role="status"/);
  assert.match(rail, /className="safety-action-error" role="alert"/);
});

test("Story 3.7 posture and command-surface wiring propagates recovery evidence to banner context", () => {
  const posture = read("apps/operator-console/src/lib/risk/posture.ts");
  const commandSurface = read(
    "apps/operator-console/src/components/risk/RiskCommandSurface.tsx",
  );

  assert.match(posture, /withRecoveryReadinessEvidence/);
  assert.match(posture, /withRecoveryResumeEvidence/);
  assert.match(posture, /resumeFailureDetails/);
  assert.match(posture, /resumeVerifiedAtIso/);
  assert.match(commandSurface, /onRecoveryEvaluated/);
  assert.match(commandSurface, /onRecoveryResumed/);
  assert.match(commandSurface, /resumeApprovedChecksum/);
});

test("Story 3.7 token styles include dedicated blocked and recovery evidence surfaces", () => {
  const globals = read("apps/operator-console/src/app/globals.css");

  assert.match(globals, /data-state="blocked-with-reasons"/);
  assert.match(globals, /\.safety-action-recovery-blocked/);
  assert.match(globals, /\.safety-action-recovery-failure/);
  assert.match(globals, /\.safety-action-recovery-evidence/);
});
