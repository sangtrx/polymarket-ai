import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 6.5 migration scope stays isolated to promotion_decisions", () => {
  const migration = read(
    "crates/persistence/migrations/20260407223000_promotion_decisions.sql",
  );

  assert.match(migration, /CREATE TABLE IF NOT EXISTS promotion_decisions/);
  assert.match(
    migration,
    /CHECK \(lifecycle_action IN \('promote', 'pause', 'retire'\)\)/,
  );
  assert.match(
    migration,
    /CHECK \(decision_state IN \('allowed', 'denied'\)\)/,
  );
  assert.match(
    migration,
    /approval_reference IS NULL[\s\S]*OR approval_request_id IS NOT NULL/s,
  );
  assert.match(migration, /idx_promotion_decisions_candidate_lookup/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS validation_runs/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS shadow_evaluations/);
});

test("Story 6.5 runbook captures FR8/FR11/FR45 flows and cross-links", () => {
  const runbook = read("docs/operations/alpha-promotion-lifecycle-governance.md");

  assert.match(runbook, /POST \/control\/research\/promotion-decisions/);
  assert.match(
    runbook,
    /GET \/control\/research\/promotion-decisions\/\{decision_id\}/,
  );
  assert.match(
    runbook,
    /GET \/control\/research\/promotion-decisions\?candidate_id=\{candidate_id\}/,
  );
  assert.match(runbook, /strategy_promotion_override/);
  assert.match(runbook, /data_quality_report/);
  assert.match(runbook, /purged_cpcv_results/);
  assert.match(runbook, /calibration_report/);
  assert.match(runbook, /counterfactual_replay_summary/);
  assert.match(runbook, /promotion_decision_dependency_unavailable/);
  assert.match(runbook, /promotion_decision_state_unavailable/);
  assert.match(runbook, /promotion_decision_persistence_unavailable/);
  assert.match(runbook, /alpha-validation-gate-policies\.md/);
  assert.match(runbook, /alpha-validation-workflow-and-diagnostics\.md/);
  assert.match(runbook, /alpha-shadow-mode-evaluation\.md/);
  assert.match(runbook, /services\/governance-service\/src\/approvals\/mod\.rs/);
});

test("Story 6.5 orchestration enforces seam reuse, FR45 completeness, and fail-closed boundaries", () => {
  const decisions = read("services/research-gateway/src/promotion/decisions.rs");

  assert.match(decisions, /evaluate_promotion_entry_gates\(/);
  assert.match(decisions, /validate_promotion_evidence_packet\(/);
  assert.match(decisions, /evaluate_promotion_thresholds\(/);
  assert.match(
    decisions,
    /PromotionDecisionReasonCode::MissingEvidence[\s\S]*\.code\(\)[\s\S]*\.to_string\(\)/s,
  );
  assert.match(
    decisions,
    /PromotionDecisionReasonCode::ThresholdFailed[\s\S]*\.code\(\)[\s\S]*\.to_string\(\)/s,
  );
  assert.match(
    decisions,
    /PromotionDecisionReasonCode::ApprovalRequired[\s\S]*\.code\(\)[\s\S]*\.to_string\(\)/s,
  );
  assert.match(
    decisions,
    /normalize_optional_timestamp\([\s\S]*"decided_after_utc"[\s\S]*input\.decided_after_utc\.as_deref\(\)[\s\S]*\)/s,
  );
  assert.match(
    decisions,
    /normalize_optional_timestamp\([\s\S]*"decided_before_utc"[\s\S]*input\.decided_before_utc\.as_deref\(\)[\s\S]*\)/s,
  );
  assert.match(
    decisions,
    /decided_before_utc must be greater than decided_after_utc/,
  );
});

test("Story 6.5 persistence adapter enforces deterministic ordering and boundary-safe filters", () => {
  const persistence = read("crates/persistence/src/postgres/promotion_decisions.rs");

  assert.match(
    persistence,
    /ORDER BY decided_at_utc DESC, decision_id ASC/,
  );
  assert.match(
    persistence,
    /if limit <= 0 \{[\s\S]*"limit must be greater than 0"/s,
  );
  assert.match(
    persistence,
    /parse_promotion_utc_timestamp\(/,
  );
});

test("Story 6.5 list orchestration preserves deterministic limit/repository contracts and allow telemetry", () => {
  const decisions = read("services/research-gateway/src/promotion/decisions.rs");

  assert.match(
    decisions,
    /let limit = input[\s\S]*\.limit[\s\S]*\.unwrap_or\(DEFAULT_LIST_LIMIT\)[\s\S]*\.clamp\(1, MAX_LIST_LIMIT\);/s,
  );
  assert.match(
    decisions,
    /list_by_candidate\([\s\S]*&normalized_candidate_id,[\s\S]*normalized_decided_after\.as_deref\(\),[\s\S]*normalized_decided_before\.as_deref\(\),[\s\S]*limit,[\s\S]*\)/s,
  );
  assert.match(
    decisions,
    /emit_promotion_decision_telemetry\([\s\S]*"promotion_decision_list_v1"[\s\S]*"promotion_decision_list"[\s\S]*"allow"[\s\S]*PromotionDecisionReasonCode::DecisionListed\.code\(\)/s,
  );
});

test("Story 6.5 list orchestration fails closed on blank canonical candidate ids", () => {
  const decisions = read("services/research-gateway/src/promotion/decisions.rs");

  assert.match(
    decisions,
    /let normalized_candidate_id = normalize_research_identifier\(&input\.candidate_id\);/,
  );
  assert.match(decisions, /if normalized_candidate_id\.is_empty\(\) \{/);
  assert.match(decisions, /candidate_id cannot be blank/);
});

test("Story 6.5 domain contract keeps deterministic FR45 and threshold semantics explicit", () => {
  const domain = read("crates/domain/src/research.rs");

  assert.match(domain, /pub const FR45_REQUIRED_PROMOTION_PACKET_FIELDS: \[&str; 4\]/);
  assert.match(
    domain,
    /"data_quality_report"[\s\S]*"purged_cpcv_results"[\s\S]*"calibration_report"[\s\S]*"counterfactual_replay_summary"/s,
  );
  assert.match(domain, /pub enum PromotionLifecycleAction \{/);
  assert.match(domain, /Self::Promote[\s\S]*Self::Pause[\s\S]*Self::Retire/s);
  assert.match(domain, /pub fn evaluate_promotion_thresholds\(/);
  assert.match(domain, /pub fn validate_promotion_evidence_packet\(/);
});

test("Story 6.5 QA command wiring executes Rust and story-scoped API/E2E checks", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-6-5"/);
  assert.match(
    packageJson,
    /cargo test -p domain research::tests::promotion_decision_/,
  );
  assert.match(
    packageJson,
    /cargo test -p persistence postgres::promotion_decisions::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p research-gateway promotion::decisions::tests::/,
  );
  assert.match(
    packageJson,
    /cargo test -p control-api routes::tests::promotion_decision_/,
  );
  assert.match(packageJson, /tests\/api\/story-6-5\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-6-5\*\.test\.mjs/);
});
