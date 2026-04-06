import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.7 control API exposes authenticated recovery evaluate/resume/query routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/recovery\/readiness\/evaluate/);
  assert.match(routes, /\/control\/recovery\/resume/);
  assert.match(routes, /\/control\/recovery\/runs\/\{run_id\}/);
  assert.match(routes, /RecoveryReadinessDecisionResponse/);
  assert.match(routes, /RecoveryResumeDecisionResponse/);
  assert.match(routes, /recovery_service_error_status/);
});

test("Story 3.7 governance recovery orchestration enforces deterministic gate semantics", () => {
  const recoveryService = read("services/governance-service/src/recovery/mod.rs");

  assert.match(recoveryService, /max\(stream_age_seconds\) <= 30/);
  assert.match(recoveryService, /mismatch_rate < 0\.1%/);
  assert.match(recoveryService, /compute_risk_limit_bundle_checksum/);
  assert.match(recoveryService, /build_operator_signoff/);
  assert.match(recoveryService, /failing_gate_codes/);
  assert.match(recoveryService, /insert_run/);
});

test("Story 3.7 migration scope is isolated to recovery_gate_runs schema contract", () => {
  const migration = read(
    "crates/persistence/migrations/20260406223000_recovery_gate_runs.sql",
  );

  assert.match(migration, /CREATE TABLE IF NOT EXISTS recovery_gate_runs/);
  assert.match(migration, /idx_recovery_gate_runs_correlation_time/);
  assert.match(migration, /idx_recovery_gate_runs_status_time/);
  assert.match(migration, /idx_recovery_gate_runs_profile_time/);
  assert.doesNotMatch(migration, /safety_control_actions/);
  assert.doesNotMatch(migration, /approval_requests/);
});

test("Story 3.7 risk-engine containment release is recovery-gated and fail-closed", () => {
  const riskEngineMain = read("services/risk-engine/src/main.rs");

  assert.match(riskEngineMain, /load_latest_recovery_gate_run/);
  assert.match(riskEngineMain, /is_approved_recovery_resume_run/);
  assert.match(riskEngineMain, /set_user_stream_auth_block\(!recovery_release_approved\)/);
});

test("Story 3.7 operator console rail supports runtime recovery states and evidence", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");
  const posture = read("apps/operator-console/src/lib/risk/posture.ts");
  const commandSurface = read(
    "apps/operator-console/src/components/risk/RiskCommandSurface.tsx",
  );

  assert.match(rail, /invokeRecoveryReadinessEvaluation/);
  assert.match(rail, /invokeRecoveryResume/);
  assert.match(rail, /blocked-with-reasons/);
  assert.match(rail, /Recovery resume blocked/);
  assert.match(rail, /Trigger:/);
  assert.match(rail, /Context:/);
  assert.match(rail, /Action:/);
  assert.match(rail, /Verification:/);
  assert.match(rail, /Controlled recovery verification/);
  assert.match(commandSurface, /onRecoveryEvaluated/);
  assert.match(commandSurface, /onRecoveryResumed/);
  assert.match(posture, /withRecoveryReadinessEvidence/);
  assert.match(posture, /withRecoveryResumeEvidence/);
});

test("Story 3.7 operations runbooks cross-link controlled recovery guidance", () => {
  const recoveryRunbook = read(
    "docs/operations/controlled-recovery-readiness-gates.md",
  );
  const emergencyRunbook = read("docs/operations/emergency-safe-state-controls.md");
  const incidentRunbook = read(
    "docs/operations/incident-search-causal-timeline-forensics.md",
  );
  const alertsRunbook = read("docs/operations/severity-alert-delivery.md");

  assert.match(recoveryRunbook, /\/control\/recovery\/readiness\/evaluate/);
  assert.match(recoveryRunbook, /\/control\/recovery\/resume/);
  assert.match(recoveryRunbook, /\/control\/recovery\/runs\/\{run_id\}/);
  assert.match(
    emergencyRunbook,
    /controlled-recovery-readiness-gates\.md/,
  );
  assert.match(
    incidentRunbook,
    /controlled-recovery-readiness-gates\.md/,
  );
  assert.match(alertsRunbook, /controlled-recovery-readiness-gates\.md/);
});
