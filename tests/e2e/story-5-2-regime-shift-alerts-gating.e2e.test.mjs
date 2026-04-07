import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 5.2 risk-engine runtime compares snapshot baselines and records FR40 detections", () => {
  const gates = read("services/risk-engine/src/gates/mod.rs");

  assert.match(gates, /evaluate_fr40_regime_shift\(&previous_snapshot, &snapshot, &correlation_id, None\)/);
  assert.match(gates, /regime_shift_detections/);
  assert.match(gates, /take_regime_shift_detections\(&self\) -> Vec<RegimeShiftDetection>/);
  assert.match(gates, /latest_regime_shift_error\(&self\) -> Option<RegimeShiftContractError>/);
});

test("Story 5.2 control API composes FR40 detection, dispatch payload, and evidence retrieval contracts", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /struct RegimeShiftAlertsQuery/);
  assert.match(routes, /struct RegimeShiftAlertDispatchPayload/);
  assert.match(routes, /evaluate_fr40_regime_shift\(/);
  assert.match(routes, /RegimeShiftAlertItem \{/);
  assert.match(routes, /recommended_next_action/);
  assert.match(routes, /evidence_link/);
  assert.match(routes, /threshold_rebate_delta_bps/);
  assert.match(routes, /threshold_spread_widening_bps/);
});

test("Story 5.2 runbook and cross-links document FR40 operator workflow continuity", () => {
  const runbook = read("docs/operations/incentive-regime-shift-alerts.md");
  const rewardRisk = read("docs/operations/reward-risk-policy-operations.md");
  const severity = read("docs/operations/severity-alert-delivery.md");

  assert.match(runbook, /GET \/control\/incidents\/regime-shifts/);
  assert.match(runbook, /POST \/control\/incidents\/regime-shifts\/dispatch/);
  assert.match(runbook, /> 20 bps/);
  assert.match(runbook, /> 50 bps/);
  assert.match(runbook, /dedupe/i);
  assert.match(runbook, /`recommended_next_action`/);
  assert.match(runbook, /`evidence_link`/);
  assert.match(rewardRisk, /incentive-regime-shift-alerts\.md/);
  assert.match(severity, /incentive-regime-shift-alerts\.md/);
});
