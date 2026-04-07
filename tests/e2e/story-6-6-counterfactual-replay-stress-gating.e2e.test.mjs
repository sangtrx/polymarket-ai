import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.6 migration scope stays isolated to counterfactual_replay_runs", () => {
  const migration = read(
    "crates/persistence/migrations/20260408010000_counterfactual_replay_runs.sql",
  );

  assert.match(
    migration,
    /CREATE TABLE IF NOT EXISTS counterfactual_replay_runs/,
  );
  assert.match(
    migration,
    /CHECK \(run_state IN \('running', 'completed', 'denied', 'failed'\)\)/,
  );
  assert.match(
    migration,
    /started_at_utc TIMESTAMPTZ NOT NULL/,
  );
  assert.match(
    migration,
    /CHECK \(EXTRACT\(TIMEZONE FROM started_at_utc\) = 0\)/,
  );
  assert.match(
    migration,
    /idx_counterfactual_replay_runs_candidate_lookup/,
  );
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS validation_runs/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS shadow_evaluations/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS promotion_decisions/);
});

test("Story 6.6 runbook captures FR46 routes, formula, fail-closed statuses, and cross-links", () => {
  const runbook = read(
    "docs/operations/alpha-counterfactual-replay-stress-gating.md",
  );

  assert.match(runbook, /POST \/control\/research\/counterfactual-replay-runs/);
  assert.match(
    runbook,
    /GET \/control\/research\/counterfactual-replay-runs\/\{replay_run_id\}/,
  );
  assert.match(
    runbook,
    /GET \/control\/research\/counterfactual-replay-runs\?candidate_id=\{candidate_id\}/,
  );
  assert.match(
    runbook,
    /degradation_pct = \(\(stressed_net_pnl - baseline_net_pnl\) \/ baseline_net_pnl\) \* 100/,
  );
  assert.match(runbook, /degradation_pct < -5\.0/);
  assert.match(runbook, /degradation_pct == -5\.0/);
  assert.match(runbook, /counterfactual_replay_invalid_payload/);
  assert.match(runbook, /counterfactual_replay_dependency_unavailable/);
  assert.match(runbook, /alpha-promotion-lifecycle-governance\.md/);
  assert.match(runbook, /alpha-validation-workflow-and-diagnostics\.md/);
  assert.match(runbook, /alpha-shadow-mode-evaluation\.md/);
});

test("Story 6.6 domain contract encodes deterministic FR46 boundary semantics and scenario parameters", () => {
  const domain = read("crates/domain/src/research.rs");

  assert.match(domain, /pub const FR46_STRESSED_SLIPPAGE_MULTIPLIER: f64 = 2\.0;/);
  assert.match(domain, /pub const FR46_STRESSED_FILL_RATE_MULTIPLIER: f64 = 0\.5;/);
  assert.match(domain, /pub const FR46_DELAYED_EXIT_SECONDS: i64 = 60;/);
  assert.match(domain, /pub const FR46_DEGRADATION_DENY_THRESHOLD_PCT: f64 = -5\.0;/);
  assert.match(
    domain,
    /degradation_pct\s*<\s*FR46_DEGRADATION_DENY_THRESHOLD_PCT/,
  );
  assert.match(
    domain,
    /CounterfactualReplayScenarioKind::StressedExecution[\s\S]*slippage_multiplier: FR46_STRESSED_SLIPPAGE_MULTIPLIER,[\s\S]*fill_rate_multiplier: FR46_STRESSED_FILL_RATE_MULTIPLIER/s,
  );
  assert.match(
    domain,
    /CounterfactualReplayScenarioKind::DelayedExit[\s\S]*exit_delay_seconds: FR46_DELAYED_EXIT_SECONDS/s,
  );
});

test("Story 6.6 replay persistence adapter enforces deterministic ordering and boundary-safe filters", () => {
  const persistence = read(
    "crates/persistence/src/postgres/counterfactual_replay_runs.rs",
  );

  assert.match(
    persistence,
    /ORDER BY started_at_utc DESC, run_id ASC/,
  );
  assert.match(
    persistence,
    /if limit <= 0 \{[\s\S]*"limit must be greater than 0"/s,
  );
  assert.match(
    persistence,
    /parse_counterfactual_replay_utc_timestamp\(/,
  );
});

test("Story 6.6 replay orchestration reuses Story 6.3 and 6.4 seams with telemetry continuity", () => {
  const replay = read(
    "services/research-gateway/src/promotion/counterfactual_replay.rs",
  );

  assert.match(replay, /pub trait ValidationEvidencePort: Send \+ Sync/);
  assert.match(replay, /pub trait ShadowEvidencePort: Send \+ Sync/);
  assert.match(replay, /load_validation_run\(&normalized_validation_run_id\)/);
  assert.match(
    replay,
    /list_validation_artifacts_by_run\(&normalized_validation_run_id\)/,
  );
  assert.match(
    replay,
    /read_latest_shadow_evaluation\([\s\S]*&normalized_candidate_id,[\s\S]*&normalized_validation_run_id,[\s\S]*\)/,
  );
  assert.match(replay, /emit_counterfactual_replay_telemetry\(/);
});

test("Story 6.6 replay list orchestration fails closed for invalid query windows with field diagnostics", () => {
  const replay = read(
    "services/research-gateway/src/promotion/counterfactual_replay.rs",
  );

  assert.match(replay, /if started_before_ts <= started_after_ts/);
  assert.match(
    replay,
    /started_before_utc must be greater than started_after_utc/,
  );
  assert.match(
    replay,
    /field: "started_before_utc"[\s\S]*code: CounterfactualReplayReasonCode::InvalidPayload\.code\(\)/s,
  );
});

test("Story 6.6 replay telemetry preserves NFR14 traceability tuple for allow and deny outcomes", () => {
  const replay = read(
    "services/research-gateway/src/promotion/counterfactual_replay.rs",
  );

  assert.match(replay, /fn emit_counterfactual_replay_telemetry\(/);
  assert.match(
    replay,
    /event_name: &'static str,[\s\S]*action: &'static str,[\s\S]*outcome: &'static str,[\s\S]*actor_id: &str,[\s\S]*candidate_id: &str,[\s\S]*run_id: Option<&str>,[\s\S]*reason_code: &str,[\s\S]*correlation_id: &str,[\s\S]*timestamp_utc: &str/s,
  );
  assert.match(
    replay,
    /struct CounterfactualReplayTelemetryEvent<'a> \{[\s\S]*event_name: &'a str,[\s\S]*action: &'a str,[\s\S]*outcome: &'a str,[\s\S]*actor_id: &'a str,[\s\S]*candidate_id: &'a str,[\s\S]*run_id: Option<&'a str>,[\s\S]*reason_code: &'a str,[\s\S]*correlation_id: &'a str,[\s\S]*timestamp_utc: &'a str,[\s\S]*\}/s,
  );
  assert.match(replay, /name: "counterfactual_replay_denied_v1"/);
});

test("Story 6.6 promotion integration backfills replay summaries and maps replay-gate denials", () => {
  const decisions = read("services/research-gateway/src/promotion/decisions.rs");
  const domain = read("crates/domain/src/research.rs");

  assert.match(
    decisions,
    /start_counterfactual_replay\(StartCounterfactualReplayInput \{/,
  );
  assert.match(
    decisions,
    /missing_evidence_fields\.retain\(\|field\| field != "counterfactual_replay_summary"\);/,
  );
  assert.match(
    decisions,
    /replay_gate_denied = replay_evidence\.replay_run\.replay_summary\.gate_outcome\s*==\s*CounterfactualReplayGateOutcome::Deny;/,
  );
  assert.match(
    decisions,
    /PromotionDecisionReasonCode::ReplayGateDenied[\s\S]*\.code\(\)[\s\S]*\.to_string\(\)/s,
  );
  assert.match(
    decisions,
    /let mut replay_run_id: Option<String> = None;[\s\S]*replay_run_id = Some\(replay_evidence\.replay_run\.run_id\.clone\(\)\);/s,
  );
  assert.match(decisions, /replay_run_id: Option<&'a str>/);
  assert.match(domain, /counterfactual_replay_placeholder_rejected/);
  assert.match(domain, /deferred_to_story_6_6/);
});

test("Story 6.6 control-api route tests include replay route coverage and unauthorized signal continuity", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /counterfactual_replay_start_route_returns_data_meta_error_envelope/);
  assert.match(routes, /counterfactual_replay_read_list_routes_return_envelope_shapes/);
  assert.match(routes, /counterfactual_replay_start_route_maps_not_found_to_conflict/);
  assert.match(routes, /unauthorized_counterfactual_replay_read_attempt_v1/);
  assert.match(routes, /unauthorized_counterfactual_replay_mutation_attempt_v1/);
});

test("Story 6.6 QA command wiring executes Rust and story-scoped API/E2E checks", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-6-6"/);
  assert.match(
    packageJson,
    /cargo test -p domain research::tests::counterfactual_replay_/,
  );
  assert.match(
    packageJson,
    /cargo test -p persistence postgres::counterfactual_replay_runs::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p research-gateway promotion::counterfactual_replay::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p control-api routes::tests::counterfactual_replay_/,
  );
  assert.match(packageJson, /tests\/api\/story-6-6\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-6-6\*\.test\.mjs/);
});
