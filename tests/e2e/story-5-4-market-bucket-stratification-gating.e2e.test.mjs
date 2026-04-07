import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 5.4 pre-trade pipeline resolves stratified policy links before exposure and FR41 checks", () => {
  const gates = read("services/risk-engine/src/gates/mod.rs");

  assert.match(gates, /resolve_pretrade_policy_context\(runtime_policy_state, intent, &evaluated_at_utc\)/);
  assert.match(gates, /let effective_profile_key = policy_context\.risk_policy_key;/);
  assert.match(gates, /evaluate_pretrade_exposure_limit_gate\(runtime_limit_state, &effective_profile_key, &evaluated_at_utc\)/);
  assert.match(gates, /evaluate_pretrade_reward_risk_gate\(runtime_policy_state, intent, &effective_profile_key\)/);
  assert.match(
    gates,
    /evaluate_pretrade_participation_guardrail_gate\(\s*runtime_policy_state,\s*runtime_limit_state,\s*intent,\s*&effective_profile_key,\s*&evaluated_at_utc,\s*\)/s,
  );
  assert.match(gates, /emit_market_bucket_resolution_telemetry\(/);
  assert.match(gates, /allocation_policy_key: resolved\.allocation_policy_key,/);
});

test("Story 5.4 stratification dependency gaps fail closed with explicit pretrade and control-uncertainty codes", () => {
  const gates = read("services/risk-engine/src/gates/mod.rs");
  const riskDomain = read("crates/domain/src/risk.rs");

  assert.match(gates, /PreTradeReasonCode::StratificationStateUnavailable/);
  assert.match(gates, /\|\s*PreTradeReasonCode::StratificationStateUnavailable/);
  assert.match(gates, /EmergencyControlTriggerSource::ControlUncertainty/);
  assert.match(gates, /EmergencyControlReasonCode::ControlUncertaintyTriggered\.code\(\)/);
  assert.match(riskDomain, /Self::StratificationStateUnavailable => "pretrade_stratification_state_unavailable"/);
  assert.match(riskDomain, /"market_bucket_mapping_unavailable"/);
  assert.match(riskDomain, /"market_bucket_mapping_conflict"/);
});

test("Story 5.4 persistence migration scope stays isolated to market_bucket_profiles", () => {
  const migration = read("crates/persistence/migrations/20260407133000_market_bucket_profiles.sql");

  assert.match(migration, /CREATE TABLE IF NOT EXISTS market_bucket_profiles/);
  assert.match(migration, /CHECK \(bucket_type IN \('core', 'satellite'\)\)/);
  assert.match(migration, /idx_market_bucket_profiles_active_market_cluster_unique/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS reward_risk_policies/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS risk_limit_profiles/);
});

test("Story 5.4 runbook and cross-links preserve FR42 operator continuity across policy seams", () => {
  const stratification = read("docs/operations/core-satellite-market-stratification.md");
  const marketPolicy = read("docs/operations/market-policy-engine.md");
  const rewardRisk = read("docs/operations/reward-risk-policy-operations.md");
  const regimeShift = read("docs/operations/incentive-regime-shift-alerts.md");
  const riskLimit = read("docs/operations/risk-limit-policy-operations.md");
  const allocation = read("docs/operations/allocation-rebalance-workflows.md");
  const pretrade = read("docs/operations/pretrade-gate-pipeline.md");

  assert.match(stratification, /POST \/control\/market-policy\/buckets\/\{market_id\}\/\{cluster_id\}/);
  assert.match(stratification, /GET \/control\/market-policy\/buckets\/\{market_id\}\/\{cluster_id\}/);
  assert.match(stratification, /bucket_type must be one of `core`, `satellite`/);
  assert.match(stratification, /`pretrade_stratification_state_unavailable`/);
  assert.match(stratification, /risk_market_bucket_resolution_v1/);
  assert.match(marketPolicy, /core-satellite-market-stratification\.md/);
  assert.match(rewardRisk, /core-satellite-market-stratification\.md/);
  assert.match(regimeShift, /core-satellite-market-stratification\.md/);
  assert.match(riskLimit, /core-satellite-market-stratification\.md/);
  assert.match(allocation, /core-satellite-market-stratification\.md/);
  assert.match(pretrade, /core-satellite-market-stratification\.md/);
});
