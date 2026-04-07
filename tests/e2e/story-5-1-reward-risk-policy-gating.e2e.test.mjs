import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 5.1 pre-trade pipeline composes reward-risk checks after venue eligibility", () => {
  const gates = read("services/risk-engine/src/gates/mod.rs");

  assert.match(gates, /evaluate_pretrade_venue_eligibility_gate\(runtime_policy_state, intent\)/);
  assert.match(gates, /evaluate_pretrade_reward_risk_gate\(runtime_policy_state, intent, &effective_profile_key\)/);
  assert.match(gates, /let reward_risk_passed = reward_risk_gate\.passed/);
  assert.match(gates, /gate_results\.push\(reward_risk_gate\)/);
  assert.match(
    gates,
    /if !reward_risk_passed \{\s*return finalize_pretrade_decision\([\s\S]*gate_results,\s*false/s,
  );
});

test("Story 5.1 reward-risk gate applies FR39 score computation and threshold comparisons deterministically", () => {
  const gates = read("services/risk-engine/src/gates/mod.rs");

  assert.match(gates, /reward_risk_score_input_from_snapshot\(&snapshot\)/);
  assert.match(gates, /compute_reward_per_risk_score\(&score_input\)/);
  assert.match(gates, /let threshold = reward_risk_threshold_for_policy\(policy_override\.as_ref\(\)\)/);
  assert.match(gates, /if score < threshold \{/);
  assert.match(gates, /PreTradeReasonCode::RewardRiskBelowThreshold/);
  assert.match(gates, /PreTradeReasonCode::Pass/);
});

test("Story 5.1 reward-risk unavailable-state denials remain fail-closed and emergency compatible", () => {
  const gates = read("services/risk-engine/src/gates/mod.rs");

  assert.match(gates, /PreTradeReasonCode::RewardRiskStateUnavailable/);
  assert.match(
    gates,
    /PreTradeReasonCode::RewardRiskStateUnavailable(?:\s*\|\s*PreTradeReasonCode::ParticipationGuardrailUnavailable)?\s*=>\s*\(\s*EmergencyControlTriggerSource::ControlUncertainty,\s*EmergencyControlReasonCode::ControlUncertaintyTriggered\.code\(\)/s,
  );
});

test("Story 5.1 runbook captures reward-risk operator API workflow and boundary semantics", () => {
  const runbook = read("docs/operations/reward-risk-policy-operations.md");

  assert.match(runbook, /POST \/control\/reward-risk\/policies\/\{policy_key\}/);
  assert.match(runbook, /GET \/control\/reward-risk\/policies\/\{policy_key\}/);
  assert.match(
    runbook,
    /\(expected_reward_bps \+ maker_rebate_bps - expected_cost_bps\) \/ expected_volatility_bps/,
  );
  assert.match(runbook, /default threshold `1\.2` is applied/);
  assert.match(runbook, /`score == threshold` passes/);
  assert.match(runbook, /`score < threshold` denies with `pretrade_reward_risk_below_threshold`/);
  assert.match(runbook, /`pretrade_reward_risk_state_unavailable`/);
});
