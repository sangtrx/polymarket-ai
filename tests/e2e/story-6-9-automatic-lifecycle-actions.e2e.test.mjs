import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.9 migration scope stays isolated to alpha_lifecycle_actions", () => {
  const migration = read(
    "crates/persistence/migrations/20260408033000_alpha_lifecycle_actions.sql",
  );

  assert.match(migration, /CREATE TABLE IF NOT EXISTS alpha_lifecycle_actions/);
  assert.match(
    migration,
    /CHECK \(action_type IN \('deallocate', 'stop_research'\)\)/,
  );
  assert.match(
    migration,
    /CHECK \(action_status IN \('applied', 'denied', 'unapplied'\)\)/,
  );
  assert.match(
    migration,
    /CHECK \(EXTRACT\(TIMEZONE FROM acted_at_utc\) = 0\)/,
  );
  assert.match(
    migration,
    /idx_alpha_lifecycle_actions_alpha_lookup/,
  );
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS promotion_decisions/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS alpha_health_metrics/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS counterfactual_replay_runs/);
});

test("Story 6.9 runbook captures lifecycle-action routes, FR47/FR48 semantics, fail-closed statuses, and cross-links", () => {
  const runbook = read(
    "docs/operations/alpha-automatic-deallocation-stop-research.md",
  );

  assert.match(runbook, /POST \/control\/research\/alpha-lifecycle-actions/);
  assert.match(
    runbook,
    /GET \/control\/research\/alpha-lifecycle-actions\/\{action_id\}/,
  );
  assert.match(
    runbook,
    /GET \/control\/research\/alpha-lifecycle-actions\?alpha_id=\{alpha_id\}/,
  );
  assert.match(runbook, /trade_count_30d < 200/);
  assert.match(runbook, /out_of_sample_sharpe_30d < 0\.2/);
  assert.match(runbook, /promotion_failure_rate_last_10 > 0\.70/);
  assert.match(runbook, /trade_count_30d == 200/);
  assert.match(runbook, /out_of_sample_sharpe_30d == 0\.2/);
  assert.match(runbook, /promotion_failure_rate_last_10 == 0\.70/);
  assert.match(runbook, /alpha_lifecycle_action_dependency_unavailable/);
  assert.match(runbook, /alpha_lifecycle_action_persistence_unavailable/);
  assert.match(runbook, /alpha-promotion-lifecycle-governance\.md/);
  assert.match(runbook, /alpha-counterfactual-replay-stress-gating\.md/);
  assert.match(runbook, /alpha-live-health-monitoring-threshold-breaches\.md/);
  assert.match(runbook, /alpha-governance-readiness-card\.md/);
});

test("Story 6.9 domain contract encodes FR47/FR48 evaluator semantics, canonical id composition, and deterministic boundaries", () => {
  const domain = read("crates/domain/src/research.rs");

  assert.match(domain, /pub enum AlphaLifecycleActionType/);
  assert.match(domain, /pub enum AlphaLifecycleActionStatus/);
  assert.match(domain, /pub enum AlphaLifecycleReasonCode/);
  assert.match(domain, /pub fn compose_alpha_lifecycle_action_id\(/);
  assert.match(domain, /pub fn evaluate_fr47_deallocation_trigger\(/);
  assert.match(domain, /pub fn evaluate_fr48_stop_research_criteria\(/);
  assert.match(
    domain,
    /trade_count_30d\s*<\s*FR48_STOP_RESEARCH_MIN_TRADE_COUNT_30D/,
  );
  assert.match(
    domain,
    /out_of_sample_sharpe_30d\s*<\s*FR48_STOP_RESEARCH_MIN_OUT_OF_SAMPLE_SHARPE_30D/,
  );
  assert.match(
    domain,
    /promotion_failure_rate_last_10\s*>\s*FR48_STOP_RESEARCH_MAX_PROMOTION_FAILURE_RATE_LAST_10/,
  );
  assert.match(
    domain,
    /alpha_lifecycle_fr48_stop_research_boundaries_keep_equality_on_allow_path/,
  );
});

test("Story 6.9 persistence adapter enforces deterministic ordering and boundary-safe window validation", () => {
  const persistence = read(
    "crates/persistence/src/postgres/alpha_lifecycle_actions.rs",
  );

  assert.match(persistence, /ORDER BY acted_at_utc DESC, action_id ASC/);
  assert.match(
    persistence,
    /acted_before_utc must be greater than acted_after_utc/,
  );
  assert.match(persistence, /parse_alpha_lifecycle_utc_timestamp\(/);
  assert.match(persistence, /pub async fn upsert_alpha_lifecycle_action/);
  assert.match(persistence, /pub async fn load_alpha_lifecycle_action/);
  assert.match(persistence, /pub async fn list_alpha_lifecycle_actions_by_alpha/);
});

test("Story 6.9 orchestration reuses Story 6.5 + 6.7 seams and preserves fail-closed dependency handling", () => {
  const lifecycle = read(
    "services/research-gateway/src/promotion/lifecycle_actions.rs",
  );

  assert.match(lifecycle, /pub trait AlphaBreachEvidencePort: Send \+ Sync/);
  assert.match(lifecycle, /pub trait PromotionHistoryPort: Send \+ Sync/);
  assert.match(
    lifecycle,
    /list_recent_breaches\(&normalized_alpha_id, history_limit\)/,
  );
  assert.match(
    lifecycle,
    /derive_promotion_failure_rate_last_10\(/,
  );
  assert.match(
    lifecycle,
    /"reflected_lifecycle_state": "deallocated"/,
  );
  assert.match(
    lifecycle,
    /alpha_lifecycle_action_start_fails_closed_when_breach_dependency_is_unavailable/,
  );
});

test("Story 6.9 FR48 promotion-history seams preserve candidate lookup and fail-closed ambiguity/incomplete-window handling", () => {
  const lifecycle = read(
    "services/research-gateway/src/promotion/lifecycle_actions.rs",
  );

  assert.match(
    lifecycle,
    /alpha_lifecycle_action_start_stop_research_supports_candidate_lookup_seam/,
  );
  assert.match(
    lifecycle,
    /alpha_lifecycle_action_start_stop_research_fails_closed_when_promotion_lookup_is_ambiguous/,
  );
  assert.match(
    lifecycle,
    /alpha_lifecycle_action_start_stop_research_fails_closed_when_promotion_window_is_incomplete/,
  );
  assert.match(
    lifecycle,
    /promotion history must include at least 10 promote decisions for stop_research evaluation/,
  );
});

test("Story 6.9 readiness derivation preserves Story 6.8 lifecycle compatibility while consuming lifecycle actions", () => {
  const readiness = read("apps/operator-console/src/lib/governance/readiness.ts");
  const runbook = read("docs/operations/alpha-governance-readiness-card.md");

  assert.match(
    readiness,
    /const alphaLifecycleActionsEndpoint = buildEndpoint\([\s\S]*\/control\/research\/alpha-lifecycle-actions/s,
  );
  assert.match(
    readiness,
    /alphaLifecycleActions: alphaLifecycleActionsResponse\.data/,
  );
  assert.match(
    readiness,
    /if \(latestAction === "retire"\) \{[\s\S]*return "deallocated";/s,
  );
  assert.match(runbook, /alpha-lifecycle-actions/);
  assert.match(
    runbook,
    /latest applied alpha lifecycle action is `deallocate`/,
  );
});

test("Story 6.9 control-api route tests and audit helpers include lifecycle identifiers and trigger evidence continuity", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /alpha_lifecycle_action_detail_response/);
  assert.match(routes, /"action_id": action_id/);
  assert.match(routes, /"criterion_count": criterion_count/);
  assert.match(routes, /alpha_lifecycle_action_start_route_returns_data_meta_error_envelope/);
  assert.match(routes, /alpha_lifecycle_action_read_list_routes_return_envelope_shapes/);
});
