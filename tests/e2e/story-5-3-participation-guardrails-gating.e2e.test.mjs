import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 5.3 risk-engine pre-trade pipeline composes FR41 gate and carries evidence", () => {
  const gates = read("services/risk-engine/src/gates/mod.rs");

  assert.match(gates, /evaluate_pretrade_participation_guardrail_gate\(/);
  assert.match(gates, /PreTradeGateDimension::ParticipationGuardrail/);
  assert.match(gates, /PreTradeReasonCode::ParticipationGuardrailLowLiquidityPause/);
  assert.match(gates, /PreTradeReasonCode::ParticipationGuardrailInactivityPause/);
  assert.match(gates, /participation_guardrail: Option<PreTradeParticipationGuardrailEvidence>/);
  assert.match(gates, /guardrail_mode: outcome\.guardrail_mode\.as_str\(\)\.to_string\(\)/);
});

test("Story 5.3 execution submit path enforces overnight size-cap denials without side effects", () => {
  const orders = read("services/execution-engine/src/orders/mod.rs");

  assert.match(orders, /requested_order_size_units: f64/);
  assert.match(orders, /participation_guardrail_size_cap_exceeded\(/);
  assert.match(
    orders,
    /PreTradeReasonCode::ParticipationGuardrailOvernightCapExceeded\.code\(\)/,
  );
  assert.match(
    orders,
    /submit_order_size_cap_exceeded_is_rejected_without_side_effects/,
  );
});

test("Story 5.3 FR41 thresholds and precedence remain deterministic for pause-versus-cap behavior", () => {
  const risk = read("crates/domain/src/risk.rs");

  assert.match(
    risk,
    /FR41_LOW_LIQUIDITY_DEPTH_USD_THRESHOLD: f64 = 10_000\.0/,
  );
  assert.match(
    risk,
    /FR41_INACTIVITY_PAUSE_THRESHOLD_SECONDS: f64 = 900\.0/,
  );
  assert.match(
    risk,
    /FR41_OVERNIGHT_CAP_THRESHOLD_SECONDS: f64 = 14_400\.0/,
  );
  assert.match(risk, /FR41_OVERNIGHT_CAP_FACTOR: f64 = 0\.25/);
  assert.match(risk, /if liquidity_depth_usd < FR41_LOW_LIQUIDITY_DEPTH_USD_THRESHOLD/);
  assert.match(
    risk,
    /else if inactivity_gap_seconds > FR41_INACTIVITY_PAUSE_THRESHOLD_SECONDS[\s\S]*inactivity_gap_seconds <= FR41_OVERNIGHT_CAP_THRESHOLD_SECONDS/s,
  );
  assert.match(risk, /else if inactivity_gap_seconds > FR41_OVERNIGHT_CAP_THRESHOLD_SECONDS/);
  assert.match(risk, /let capped = normal_baseline \* FR41_OVERNIGHT_CAP_FACTOR/);
});

test("Story 5.3 FR41 runbook and cross-links preserve operator continuity", () => {
  const runbook = read("docs/operations/low-liquidity-overnight-guardrails.md");
  const pretrade = read("docs/operations/pretrade-gate-pipeline.md");
  const riskLimits = read("docs/operations/risk-limit-policy-operations.md");
  const regimeShift = read("docs/operations/incentive-regime-shift-alerts.md");

  assert.match(runbook, /GET \/control\/incidents\/participation-guardrails/);
  assert.match(runbook, /liquidity_depth_usd < 10_000/);
  assert.match(runbook, /inactivity_gap_seconds > 900/);
  assert.match(runbook, /inactivity_gap_seconds > 14_400/);
  assert.match(runbook, /capped_max_order_size_units = normal_max_order_size_units \* 0\.25/);
  assert.match(runbook, /pretrade_participation_guardrail_low_liquidity_pause/);
  assert.match(pretrade, /low-liquidity-overnight-guardrails\.md/);
  assert.match(riskLimits, /low-liquidity-overnight-guardrails\.md/);
  assert.match(regimeShift, /low-liquidity-overnight-guardrails\.md/);
});
