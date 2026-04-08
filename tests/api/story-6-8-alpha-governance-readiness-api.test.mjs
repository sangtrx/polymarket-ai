import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { rmSync, mkdtempSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const PROJECT_ROOT = resolve(".");
const OPERATOR_CONSOLE_FILTER = "operator-console";
const READINESS_SOURCE_PATH = "src/lib/governance/readiness.ts";

let compiledOutputDir;
let readinessModule;

function findFileRecursively(rootDir, fileName) {
  const queue = [rootDir];
  while (queue.length > 0) {
    const nextDir = queue.shift();
    const entries = readdirSync(nextDir, { withFileTypes: true });
    for (const entry of entries) {
      const entryPath = join(nextDir, entry.name);
      if (entry.isDirectory()) {
        queue.push(entryPath);
      } else if (entry.isFile() && entry.name === fileName) {
        return entryPath;
      }
    }
  }
  return undefined;
}

function loadCompiledModule() {
  if (readinessModule) {
    return readinessModule;
  }

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-6-8-governance-readiness-api-"));
  const compileResult = spawnSync(
    "pnpm",
    [
      "--filter",
      OPERATOR_CONSOLE_FILTER,
      "exec",
      "tsc",
      "--pretty",
      "false",
      "--module",
      "commonjs",
      "--target",
      "es2022",
      "--outDir",
      compiledOutputDir,
      READINESS_SOURCE_PATH,
    ],
    { cwd: PROJECT_ROOT, encoding: "utf8" },
  );
  assert.equal(compileResult.status, 0, compileResult.stderr || compileResult.stdout);

  const compiledPath = findFileRecursively(compiledOutputDir, "readiness.js");
  assert.ok(compiledPath, "expected compiled readiness.js output");

  const require = createRequire(import.meta.url);
  readinessModule = require(compiledPath);
  return readinessModule;
}

test.after(() => {
  if (compiledOutputDir) {
    rmSync(compiledOutputDir, { recursive: true, force: true });
  }
});

function successEnvelope(data, endpoint, action = "governance_readiness_query") {
  return {
    data,
    meta: {
      action,
      actor_id: "ops-1",
      role: "operational_control",
      correlation_id: "corr-governance-readiness-001",
      timestamp_utc: "2026-04-08T08:00:00.000Z",
      endpoint,
    },
    error: null,
  };
}

test("Story 6.8 API composes research reads and derives deterministic readiness signals", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();
  const requests = [];
  const endpointPayloads = new Map([
    [
      "/control/research/promotion-decisions",
      successEnvelope(
        {
          kind: "decisions",
          candidate_id: "candidate::alpha-1",
          decisions: [
            {
              decision_id: "decision-003",
              candidate_id: "candidate::alpha-1",
              validation_run_id: "run-003",
              lifecycle_action: "promote",
              decision_state: "allowed",
              reason_code: "promotion_decision_allowed",
              observed_metrics: {},
              evidence_packet: {
                data_quality_report: { artifact_id: "quality::003" },
                purged_cpcv_results: { artifact_id: "cpcv::003" },
                calibration_report: { artifact_id: "calibration::003" },
                counterfactual_replay_summary: { run_id: "replay::003" },
              },
              threshold_results: [],
              missing_evidence_fields: [],
              gate_evaluation: { outcome: "allow" },
              shadow_readiness: { reason_code: "shadow_ready" },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              decided_at_utc: "2026-04-08T07:50:00.000Z",
            },
            {
              decision_id: "decision-002",
              candidate_id: "candidate::alpha-1",
              validation_run_id: "run-002",
              lifecycle_action: "promote",
              decision_state: "allowed",
              reason_code: "promotion_decision_allowed",
              observed_metrics: {},
              evidence_packet: {
                data_quality_report: { artifact_id: "quality::002" },
                purged_cpcv_results: { artifact_id: "cpcv::002" },
                calibration_report: { artifact_id: "calibration::002" },
                counterfactual_replay_summary: { run_id: "replay::002" },
              },
              threshold_results: [],
              missing_evidence_fields: [],
              gate_evaluation: { outcome: "allow" },
              shadow_readiness: { reason_code: "shadow_ready" },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              decided_at_utc: "2026-04-08T07:30:00.000Z",
            },
          ],
        },
        "/control/research/promotion-decisions",
      ),
    ],
    [
      "/control/research/shadow-evaluations",
      successEnvelope(
        {
          kind: "evaluations",
          candidate_id: "candidate::alpha-1",
          evaluations: [
            {
              evaluation_id: "shadow-001",
              candidate_id: "candidate::alpha-1",
              validation_run_id: "run-003",
              evaluation_state: "completed",
              reason_code: "shadow_completed",
              market_context: {},
              signal_decisions: {},
              simulation_outcomes: [
                {
                  decision_side: "buy",
                  intended_size: 100,
                  simulated_fill_size: 100,
                  simulated_fill_price: 1.11,
                  simulated_slippage_bps: 4.2,
                  simulation_reason_code: "shadow_fill_simulated",
                  decision_timestamp_utc: "2026-04-08T07:45:00.000Z",
                  simulated_at_utc: "2026-04-08T07:45:01.000Z",
                },
              ],
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              started_at_utc: "2026-04-08T07:40:00.000Z",
              completed_at_utc: "2026-04-08T07:46:00.000Z",
            },
          ],
        },
        "/control/research/shadow-evaluations",
      ),
    ],
    [
      "/control/research/validation-runs",
      successEnvelope(
        {
          kind: "runs",
          candidate_id: "candidate::alpha-1",
          runs: [
            {
              run_id: "run-003",
              candidate_id: "candidate::alpha-1",
              run_state: "completed",
              reason_code: "validation_run_completed",
              gate_evaluation: { outcome: "allow" },
              comparison_ready: true,
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              started_at_utc: "2026-04-08T06:00:00.000Z",
              completed_at_utc: "2026-04-08T06:30:00.000Z",
            },
          ],
        },
        "/control/research/validation-runs",
      ),
    ],
    [
      "/control/research/validation-runs/run-003",
      successEnvelope(
        {
          kind: "run_detail",
          run: {
            run_id: "run-003",
            candidate_id: "candidate::alpha-1",
            run_state: "completed",
            reason_code: "validation_run_completed",
            gate_evaluation: { outcome: "allow" },
            comparison_ready: true,
            actor_id: "ops-1",
            correlation_id: "corr-governance-readiness-001",
            started_at_utc: "2026-04-08T06:00:00.000Z",
            completed_at_utc: "2026-04-08T06:30:00.000Z",
          },
          artifacts: [
            {
              artifact_id: "artifact-quality",
              run_id: "run-003",
              candidate_id: "candidate::alpha-1",
              stage: "quality",
              stage_index: 1,
              stage_outcome: "passed",
              reason_code: "validation_stage_passed",
              diagnostics: {
                out_of_sample_sharpe: 1.3,
                max_drawdown: -0.1,
                overfit_indicator: 0.2,
                overfit_flag: false,
              },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              stage_started_at_utc: "2026-04-08T06:00:00.000Z",
              stage_completed_at_utc: "2026-04-08T06:05:00.000Z",
            },
            {
              artifact_id: "artifact-labeling",
              run_id: "run-003",
              candidate_id: "candidate::alpha-1",
              stage: "labeling",
              stage_index: 2,
              stage_outcome: "passed",
              reason_code: "validation_stage_passed",
              diagnostics: {
                out_of_sample_sharpe: 1.3,
                max_drawdown: -0.1,
                overfit_indicator: 0.2,
                overfit_flag: false,
              },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              stage_started_at_utc: "2026-04-08T06:05:00.000Z",
              stage_completed_at_utc: "2026-04-08T06:10:00.000Z",
            },
            {
              artifact_id: "artifact-purged-cv",
              run_id: "run-003",
              candidate_id: "candidate::alpha-1",
              stage: "purged_cv",
              stage_index: 3,
              stage_outcome: "passed",
              reason_code: "validation_stage_passed",
              diagnostics: {
                out_of_sample_sharpe: 1.3,
                max_drawdown: -0.1,
                overfit_indicator: 0.2,
                overfit_flag: false,
              },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              stage_started_at_utc: "2026-04-08T06:10:00.000Z",
              stage_completed_at_utc: "2026-04-08T06:15:00.000Z",
            },
            {
              artifact_id: "artifact-cpcv",
              run_id: "run-003",
              candidate_id: "candidate::alpha-1",
              stage: "cpcv",
              stage_index: 4,
              stage_outcome: "passed",
              reason_code: "validation_stage_passed",
              diagnostics: {
                out_of_sample_sharpe: 1.3,
                max_drawdown: -0.1,
                overfit_indicator: 0.2,
                overfit_flag: false,
              },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              stage_started_at_utc: "2026-04-08T06:15:00.000Z",
              stage_completed_at_utc: "2026-04-08T06:20:00.000Z",
            },
            {
              artifact_id: "artifact-overfit",
              run_id: "run-003",
              candidate_id: "candidate::alpha-1",
              stage: "overfit_diagnostics",
              stage_index: 5,
              stage_outcome: "passed",
              reason_code: "validation_stage_passed",
              diagnostics: {
                out_of_sample_sharpe: 1.3,
                max_drawdown: -0.1,
                overfit_indicator: 0.2,
                overfit_flag: false,
              },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              stage_started_at_utc: "2026-04-08T06:20:00.000Z",
              stage_completed_at_utc: "2026-04-08T06:25:00.000Z",
            },
          ],
          comparisons: [],
        },
        "/control/research/validation-runs/run-003",
      ),
    ],
    [
      "/control/research/alpha-health-metrics",
      successEnvelope(
        {
          kind: "metrics",
          alpha_id: "alpha::mean-reversion",
          metrics: [
            {
              metric_id: "metric-001",
              alpha_id: "alpha::mean-reversion",
              rolling_sharpe: 1.1,
              rolling_hit_rate: 0.58,
              rolling_drawdown: 0.09,
              stability_score: 0.85,
              windows: [
                {
                  window: "1h",
                  net_pnl: 12.3,
                  rolling_sharpe: 1.12,
                  rolling_hit_rate: 0.59,
                  rolling_drawdown: 0.08,
                  stability_score: 0.86,
                },
                {
                  window: "24h",
                  net_pnl: 51.7,
                  rolling_sharpe: 1.1,
                  rolling_hit_rate: 0.58,
                  rolling_drawdown: 0.09,
                  stability_score: 0.85,
                },
                {
                  window: "30d",
                  net_pnl: 204.9,
                  rolling_sharpe: 1.07,
                  rolling_hit_rate: 0.57,
                  rolling_drawdown: 0.1,
                  stability_score: 0.83,
                },
              ],
              reason_code: "alpha_health_ready",
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              recorded_at_utc: "2026-04-08T07:55:00.000Z",
            },
          ],
        },
        "/control/research/alpha-health-metrics",
      ),
    ],
    [
      "/control/research/alpha-threshold-breaches",
      successEnvelope(
        {
          kind: "breaches",
          alpha_id: "alpha::mean-reversion",
          breaches: [],
        },
        "/control/research/alpha-threshold-breaches",
      ),
    ],
    [
      "/control/research/alpha-lifecycle-actions",
      successEnvelope(
        {
          kind: "actions",
          alpha_id: "alpha::mean-reversion",
          actions: [],
        },
        "/control/research/alpha-lifecycle-actions",
      ),
    ],
  ]);

  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    const parsed = new URL(url);
    const endpointKey =
      parsed.pathname === "/control/research/validation-runs/run-003"
        ? "/control/research/validation-runs/run-003"
        : parsed.pathname;
    const payload = endpointPayloads.get(endpointKey);
    assert.ok(payload, `Unexpected endpoint request: ${parsed.pathname}`);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  };

  const result = await queryAlphaGovernanceReadiness({
    baseUrl: "http://127.0.0.1:8080/",
    candidateId: " Candidate::Alpha-1 ",
    alphaId: " Alpha::Mean-Reversion ",
    fetchImpl,
  });

  assert.equal(requests.length, 7);
  assert.equal(result.candidateId, "candidate::alpha-1");
  assert.equal(result.alphaId, "alpha::mean-reversion");
  assert.equal(result.lifecycleState, "production");
  assert.equal(result.validationCompleteness, "complete");
  assert.equal(result.shadowStability, "stable");
  assert.equal(result.guardrailState, "healthy");
  assert.equal(result.readinessStatus, "ready");
  assert.deepEqual(result.missingArtifacts, []);
});

test("Story 6.8 API derives blocked readiness with explicit missing-artifact evidence", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();
  const endpointPayloads = new Map([
    [
      "/control/research/promotion-decisions",
      successEnvelope(
        {
          kind: "decisions",
          candidate_id: "candidate::alpha-1",
          decisions: [
            {
              decision_id: "decision-004",
              candidate_id: "candidate::alpha-1",
              validation_run_id: "run-004",
              lifecycle_action: "promote",
              decision_state: "allowed",
              reason_code: "promotion_decision_allowed",
              observed_metrics: {},
              evidence_packet: {
                data_quality_report: { artifact_id: "quality::004" },
              },
              threshold_results: [],
              missing_evidence_fields: [
                "purged_cpcv_results",
                "calibration_report",
                "counterfactual_replay_summary",
              ],
              gate_evaluation: { outcome: "allow" },
              shadow_readiness: { reason_code: "shadow_partial" },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              decided_at_utc: "2026-04-08T07:20:00.000Z",
            },
          ],
        },
        "/control/research/promotion-decisions",
      ),
    ],
    [
      "/control/research/shadow-evaluations",
      successEnvelope(
        {
          kind: "evaluations",
          candidate_id: "candidate::alpha-1",
          evaluations: [
            {
              evaluation_id: "shadow-004",
              candidate_id: "candidate::alpha-1",
              validation_run_id: "run-004",
              evaluation_state: "completed",
              reason_code: "shadow_completed_without_simulation_outcomes",
              market_context: {},
              signal_decisions: {},
              simulation_outcomes: [],
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              started_at_utc: "2026-04-08T07:00:00.000Z",
              completed_at_utc: "2026-04-08T07:10:00.000Z",
            },
          ],
        },
        "/control/research/shadow-evaluations",
      ),
    ],
    [
      "/control/research/validation-runs",
      successEnvelope(
        {
          kind: "runs",
          candidate_id: "candidate::alpha-1",
          runs: [
            {
              run_id: "run-004",
              candidate_id: "candidate::alpha-1",
              run_state: "completed",
              reason_code: "validation_run_completed",
              gate_evaluation: { outcome: "allow" },
              comparison_ready: true,
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              started_at_utc: "2026-04-08T06:00:00.000Z",
              completed_at_utc: "2026-04-08T06:30:00.000Z",
            },
          ],
        },
        "/control/research/validation-runs",
      ),
    ],
    [
      "/control/research/validation-runs/run-004",
      successEnvelope(
        {
          kind: "run_detail",
          run: {
            run_id: "run-004",
            candidate_id: "candidate::alpha-1",
            run_state: "completed",
            reason_code: "validation_run_completed",
            gate_evaluation: { outcome: "allow" },
            comparison_ready: true,
            actor_id: "ops-1",
            correlation_id: "corr-governance-readiness-001",
            started_at_utc: "2026-04-08T06:00:00.000Z",
            completed_at_utc: "2026-04-08T06:30:00.000Z",
          },
          artifacts: [
            {
              artifact_id: "artifact-quality-004",
              run_id: "run-004",
              candidate_id: "candidate::alpha-1",
              stage: "quality",
              stage_index: 1,
              stage_outcome: "passed",
              reason_code: "validation_stage_passed",
              diagnostics: {
                out_of_sample_sharpe: 1.1,
                max_drawdown: -0.11,
                overfit_indicator: 0.21,
                overfit_flag: false,
              },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              stage_started_at_utc: "2026-04-08T06:00:00.000Z",
              stage_completed_at_utc: "2026-04-08T06:05:00.000Z",
            },
          ],
          comparisons: [],
        },
        "/control/research/validation-runs/run-004",
      ),
    ],
    [
      "/control/research/alpha-health-metrics",
      successEnvelope(
        {
          kind: "metrics",
          alpha_id: "alpha::mean-reversion",
          metrics: [
            {
              metric_id: "metric-004",
              alpha_id: "alpha::mean-reversion",
              rolling_sharpe: 1.02,
              rolling_hit_rate: 0.56,
              rolling_drawdown: 0.11,
              stability_score: 0.79,
              windows: [
                {
                  window: "24h",
                  net_pnl: 39.4,
                  rolling_sharpe: 1.02,
                  rolling_hit_rate: 0.56,
                  rolling_drawdown: 0.11,
                  stability_score: 0.79,
                },
              ],
              reason_code: "alpha_health_partial",
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              recorded_at_utc: "2026-04-08T07:25:00.000Z",
            },
          ],
        },
        "/control/research/alpha-health-metrics",
      ),
    ],
    [
      "/control/research/alpha-threshold-breaches",
      successEnvelope(
        {
          kind: "breaches",
          alpha_id: "alpha::mean-reversion",
          breaches: [],
        },
        "/control/research/alpha-threshold-breaches",
      ),
    ],
    [
      "/control/research/alpha-lifecycle-actions",
      successEnvelope(
        {
          kind: "actions",
          alpha_id: "alpha::mean-reversion",
          actions: [],
        },
        "/control/research/alpha-lifecycle-actions",
      ),
    ],
  ]);

  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const endpointKey =
      parsed.pathname === "/control/research/validation-runs/run-004"
        ? "/control/research/validation-runs/run-004"
        : parsed.pathname;
    const payload = endpointPayloads.get(endpointKey);
    assert.ok(payload, `Unexpected endpoint request: ${parsed.pathname}`);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  };

  const result = await queryAlphaGovernanceReadiness({
    baseUrl: "http://127.0.0.1:8080",
    candidateId: "candidate::alpha-1",
    alphaId: "alpha::mean-reversion",
    fetchImpl,
  });

  assert.equal(result.lifecycleState, "candidate-live");
  assert.equal(result.validationCompleteness, "incomplete");
  assert.equal(result.shadowStability, "unstable");
  assert.equal(result.guardrailState, "missing");
  assert.equal(result.readinessStatus, "blocked");
  assert.ok(result.missingArtifacts.includes("validation_stage:labeling"));
  assert.ok(result.missingArtifacts.includes("validation_stage:purged_cv"));
  assert.ok(result.missingArtifacts.includes("validation_stage:cpcv"));
  assert.ok(result.missingArtifacts.includes("validation_stage:overfit_diagnostics"));
  assert.ok(
    result.missingArtifacts.includes("promotion_packet_field:purged_cpcv_results"),
  );
  assert.ok(result.missingArtifacts.includes("promotion_packet_field:calibration_report"));
  assert.ok(
    result.missingArtifacts.includes(
      "promotion_packet_field:counterfactual_replay_summary",
    ),
  );
  assert.ok(result.missingArtifacts.includes("shadow_stability_evidence"));
  assert.ok(result.missingArtifacts.includes("alpha_health_window:1h"));
  assert.ok(result.missingArtifacts.includes("alpha_health_window:30d"));
  assert.match(result.recommendedNextAction, /missing FR7 validation stages/);
});

test("Story 6.8 API reflects applied stop_research lifecycle actions as deallocated", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();
  const endpointPayloads = new Map([
    [
      "/control/research/promotion-decisions",
      successEnvelope(
        {
          kind: "decisions",
          candidate_id: "candidate::alpha-1",
          decisions: [
            {
              decision_id: "decision-stop-research-001",
              candidate_id: "candidate::alpha-1",
              validation_run_id: "run-stop-research-001",
              lifecycle_action: "promote",
              decision_state: "allowed",
              reason_code: "promotion_decision_allowed",
              observed_metrics: {},
              evidence_packet: {},
              threshold_results: [],
              missing_evidence_fields: [],
              gate_evaluation: { outcome: "allow" },
              shadow_readiness: { reason_code: "shadow_ready" },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              decided_at_utc: "2026-04-08T09:00:00.000Z",
            },
          ],
        },
        "/control/research/promotion-decisions",
      ),
    ],
    [
      "/control/research/shadow-evaluations",
      successEnvelope(
        {
          kind: "evaluations",
          candidate_id: "candidate::alpha-1",
          evaluations: [],
        },
        "/control/research/shadow-evaluations",
      ),
    ],
    [
      "/control/research/validation-runs",
      successEnvelope(
        {
          kind: "runs",
          candidate_id: "candidate::alpha-1",
          runs: [],
        },
        "/control/research/validation-runs",
      ),
    ],
    [
      "/control/research/alpha-health-metrics",
      successEnvelope(
        {
          kind: "metrics",
          alpha_id: "alpha::mean-reversion",
          metrics: [],
        },
        "/control/research/alpha-health-metrics",
      ),
    ],
    [
      "/control/research/alpha-threshold-breaches",
      successEnvelope(
        {
          kind: "breaches",
          alpha_id: "alpha::mean-reversion",
          breaches: [],
        },
        "/control/research/alpha-threshold-breaches",
      ),
    ],
    [
      "/control/research/alpha-lifecycle-actions",
      successEnvelope(
        {
          kind: "actions",
          alpha_id: "alpha::mean-reversion",
          actions: [
            {
              action_id: "action-stop-research-001",
              alpha_id: "alpha::mean-reversion",
              action_type: "stop_research",
              action_status: "applied",
              reason_code: "alpha_lifecycle_action_stop_research_criteria_met",
              correlation_id: "corr-stop-research-001",
              acted_at_utc: "2026-04-08T09:59:00.000Z",
            },
          ],
        },
        "/control/research/alpha-lifecycle-actions",
      ),
    ],
  ]);

  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const payload = endpointPayloads.get(parsed.pathname);
    assert.ok(payload, `Unexpected endpoint request: ${parsed.pathname}`);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  };

  const result = await queryAlphaGovernanceReadiness({
    baseUrl: "http://127.0.0.1:8080",
    candidateId: "candidate::alpha-1",
    alphaId: "alpha::mean-reversion",
    fetchImpl,
  });

  assert.equal(result.lifecycleState, "deallocated");
  assert.equal(result.asOfUtc, "2026-04-08T09:59:00.000Z");
});

test("Story 6.8 API uses newest lifecycle event when promotion is newer than deallocation", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();
  const endpointPayloads = new Map([
    [
      "/control/research/promotion-decisions",
      successEnvelope(
        {
          kind: "decisions",
          candidate_id: "candidate::alpha-1",
          decisions: [
            {
              decision_id: "decision-newer-promote-001",
              candidate_id: "candidate::alpha-1",
              validation_run_id: "run-newer-promote-001",
              lifecycle_action: "promote",
              decision_state: "allowed",
              reason_code: "promotion_decision_allowed",
              observed_metrics: {},
              evidence_packet: {},
              threshold_results: [],
              missing_evidence_fields: [],
              gate_evaluation: { outcome: "allow" },
              shadow_readiness: { reason_code: "shadow_ready" },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              decided_at_utc: "2026-04-08T09:00:00.000Z",
            },
          ],
        },
        "/control/research/promotion-decisions",
      ),
    ],
    [
      "/control/research/shadow-evaluations",
      successEnvelope(
        {
          kind: "evaluations",
          candidate_id: "candidate::alpha-1",
          evaluations: [],
        },
        "/control/research/shadow-evaluations",
      ),
    ],
    [
      "/control/research/validation-runs",
      successEnvelope(
        {
          kind: "runs",
          candidate_id: "candidate::alpha-1",
          runs: [],
        },
        "/control/research/validation-runs",
      ),
    ],
    [
      "/control/research/alpha-health-metrics",
      successEnvelope(
        {
          kind: "metrics",
          alpha_id: "alpha::mean-reversion",
          metrics: [],
        },
        "/control/research/alpha-health-metrics",
      ),
    ],
    [
      "/control/research/alpha-threshold-breaches",
      successEnvelope(
        {
          kind: "breaches",
          alpha_id: "alpha::mean-reversion",
          breaches: [],
        },
        "/control/research/alpha-threshold-breaches",
      ),
    ],
    [
      "/control/research/alpha-lifecycle-actions",
      successEnvelope(
        {
          kind: "actions",
          alpha_id: "alpha::mean-reversion",
          actions: [
            {
              action_id: "action-deallocate-older-001",
              alpha_id: "alpha::mean-reversion",
              action_type: "deallocate",
              action_status: "applied",
              reason_code: "alpha_lifecycle_action_deallocation_threshold_breached",
              correlation_id: "corr-deallocate-older-001",
              acted_at_utc: "2026-04-08T08:00:00.000Z",
            },
          ],
        },
        "/control/research/alpha-lifecycle-actions",
      ),
    ],
  ]);

  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const payload = endpointPayloads.get(parsed.pathname);
    assert.ok(payload, `Unexpected endpoint request: ${parsed.pathname}`);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  };

  const result = await queryAlphaGovernanceReadiness({
    baseUrl: "http://127.0.0.1:8080",
    candidateId: "candidate::alpha-1",
    alphaId: "alpha::mean-reversion",
    fetchImpl,
  });

  assert.equal(result.lifecycleState, "candidate-live");
});

test("Story 6.8 API does not retain deallocated state when a newer lifecycle action is unapplied", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();
  const endpointPayloads = new Map([
    [
      "/control/research/promotion-decisions",
      successEnvelope(
        {
          kind: "decisions",
          candidate_id: "candidate::alpha-1",
          decisions: [
            {
              decision_id: "decision-unapplied-001",
              candidate_id: "candidate::alpha-1",
              validation_run_id: "run-unapplied-001",
              lifecycle_action: "promote",
              decision_state: "allowed",
              reason_code: "promotion_decision_allowed",
              observed_metrics: {},
              evidence_packet: {},
              threshold_results: [],
              missing_evidence_fields: [],
              gate_evaluation: { outcome: "allow" },
              shadow_readiness: { reason_code: "shadow_ready" },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              decided_at_utc: "2026-04-08T09:00:00.000Z",
            },
          ],
        },
        "/control/research/promotion-decisions",
      ),
    ],
    [
      "/control/research/shadow-evaluations",
      successEnvelope(
        {
          kind: "evaluations",
          candidate_id: "candidate::alpha-1",
          evaluations: [],
        },
        "/control/research/shadow-evaluations",
      ),
    ],
    [
      "/control/research/validation-runs",
      successEnvelope(
        {
          kind: "runs",
          candidate_id: "candidate::alpha-1",
          runs: [],
        },
        "/control/research/validation-runs",
      ),
    ],
    [
      "/control/research/alpha-health-metrics",
      successEnvelope(
        {
          kind: "metrics",
          alpha_id: "alpha::mean-reversion",
          metrics: [],
        },
        "/control/research/alpha-health-metrics",
      ),
    ],
    [
      "/control/research/alpha-threshold-breaches",
      successEnvelope(
        {
          kind: "breaches",
          alpha_id: "alpha::mean-reversion",
          breaches: [],
        },
        "/control/research/alpha-threshold-breaches",
      ),
    ],
    [
      "/control/research/alpha-lifecycle-actions",
      successEnvelope(
        {
          kind: "actions",
          alpha_id: "alpha::mean-reversion",
          actions: [
            {
              action_id: "action-deallocate-applied-001",
              alpha_id: "alpha::mean-reversion",
              action_type: "deallocate",
              action_status: "applied",
              reason_code: "alpha_lifecycle_action_deallocation_threshold_breached",
              correlation_id: "corr-deallocate-applied-001",
              acted_at_utc: "2026-04-08T09:59:00.000Z",
            },
            {
              action_id: "action-deallocate-unapplied-001",
              alpha_id: "alpha::mean-reversion",
              action_type: "deallocate",
              action_status: "unapplied",
              reason_code: "alpha_lifecycle_action_authorization_failed",
              correlation_id: "corr-deallocate-unapplied-001",
              acted_at_utc: "2026-04-08T10:05:00.000Z",
            },
          ],
        },
        "/control/research/alpha-lifecycle-actions",
      ),
    ],
  ]);

  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const payload = endpointPayloads.get(parsed.pathname);
    assert.ok(payload, `Unexpected endpoint request: ${parsed.pathname}`);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  };

  const result = await queryAlphaGovernanceReadiness({
    baseUrl: "http://127.0.0.1:8080",
    candidateId: "candidate::alpha-1",
    alphaId: "alpha::mean-reversion",
    fetchImpl,
  });

  assert.equal(result.lifecycleState, "candidate-live");
  assert.equal(result.asOfUtc, "2026-04-08T10:05:00.000Z");
});

test("Story 6.8 API fails closed when lifecycle-action read endpoint is unavailable", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();
  const endpointPayloads = new Map([
    [
      "/control/research/promotion-decisions",
      successEnvelope(
        {
          kind: "decisions",
          candidate_id: "candidate::alpha-1",
          decisions: [
            {
              decision_id: "decision-compat-001",
              candidate_id: "candidate::alpha-1",
              validation_run_id: "run-compat-001",
              lifecycle_action: "promote",
              decision_state: "allowed",
              reason_code: "promotion_decision_allowed",
              observed_metrics: {},
              evidence_packet: {},
              threshold_results: [],
              missing_evidence_fields: [],
              gate_evaluation: { outcome: "allow" },
              shadow_readiness: { reason_code: "shadow_ready" },
              actor_id: "ops-1",
              correlation_id: "corr-governance-readiness-001",
              decided_at_utc: "2026-04-08T09:00:00.000Z",
            },
          ],
        },
        "/control/research/promotion-decisions",
      ),
    ],
    [
      "/control/research/shadow-evaluations",
      successEnvelope(
        {
          kind: "evaluations",
          candidate_id: "candidate::alpha-1",
          evaluations: [],
        },
        "/control/research/shadow-evaluations",
      ),
    ],
    [
      "/control/research/validation-runs",
      successEnvelope(
        {
          kind: "runs",
          candidate_id: "candidate::alpha-1",
          runs: [],
        },
        "/control/research/validation-runs",
      ),
    ],
    [
      "/control/research/alpha-health-metrics",
      successEnvelope(
        {
          kind: "metrics",
          alpha_id: "alpha::mean-reversion",
          metrics: [],
        },
        "/control/research/alpha-health-metrics",
      ),
    ],
    [
      "/control/research/alpha-threshold-breaches",
      successEnvelope(
        {
          kind: "breaches",
          alpha_id: "alpha::mean-reversion",
          breaches: [],
        },
        "/control/research/alpha-threshold-breaches",
      ),
    ],
  ]);

  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    if (parsed.pathname === "/control/research/alpha-lifecycle-actions") {
      return {
        ok: false,
        status: 404,
        json: async () => ({
          data: null,
          meta: {
            action: "governance_readiness_query",
            actor_id: "ops-1",
            role: "operational_control",
            correlation_id: "corr-governance-readiness-001",
            timestamp_utc: "2026-04-08T08:00:00.000Z",
            endpoint: parsed.pathname,
          },
          error: {
            error_code: "alpha_lifecycle_action_not_found",
            message: "alpha lifecycle actions endpoint unavailable",
          },
        }),
      };
    }

    const payload = endpointPayloads.get(parsed.pathname);
    assert.ok(payload, `Unexpected endpoint request: ${parsed.pathname}`);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  };

  await assert.rejects(
    queryAlphaGovernanceReadiness({
      baseUrl: "http://127.0.0.1:8080",
      candidateId: "candidate::alpha-1",
      alphaId: "alpha::mean-reversion",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 404);
      assert.equal(error.errorCode, "alpha_lifecycle_action_not_found");
      return true;
    },
  );
});

test("Story 6.8 API fails closed when lifecycle-action dependency is unavailable", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();
  const endpointPayloads = new Map([
    [
      "/control/research/promotion-decisions",
      successEnvelope(
        {
          kind: "decisions",
          candidate_id: "candidate::alpha-1",
          decisions: [],
        },
        "/control/research/promotion-decisions",
      ),
    ],
    [
      "/control/research/shadow-evaluations",
      successEnvelope(
        {
          kind: "evaluations",
          candidate_id: "candidate::alpha-1",
          evaluations: [],
        },
        "/control/research/shadow-evaluations",
      ),
    ],
    [
      "/control/research/validation-runs",
      successEnvelope(
        {
          kind: "runs",
          candidate_id: "candidate::alpha-1",
          runs: [],
        },
        "/control/research/validation-runs",
      ),
    ],
    [
      "/control/research/alpha-health-metrics",
      successEnvelope(
        {
          kind: "metrics",
          alpha_id: "alpha::mean-reversion",
          metrics: [],
        },
        "/control/research/alpha-health-metrics",
      ),
    ],
    [
      "/control/research/alpha-threshold-breaches",
      successEnvelope(
        {
          kind: "breaches",
          alpha_id: "alpha::mean-reversion",
          breaches: [],
        },
        "/control/research/alpha-threshold-breaches",
      ),
    ],
  ]);

  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    if (parsed.pathname === "/control/research/alpha-lifecycle-actions") {
      return {
        ok: false,
        status: 503,
        json: async () => ({
          data: null,
          meta: {
            action: "governance_readiness_query",
            actor_id: "ops-1",
            role: "operational_control",
            correlation_id: "corr-governance-readiness-001",
            timestamp_utc: "2026-04-08T08:00:00.000Z",
            endpoint: parsed.pathname,
          },
          error: {
            error_code: "alpha_lifecycle_action_dependency_unavailable",
            message: "alpha lifecycle dependencies unavailable",
          },
        }),
      };
    }

    const payload = endpointPayloads.get(parsed.pathname);
    assert.ok(payload, `Unexpected endpoint request: ${parsed.pathname}`);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  };

  await assert.rejects(
    queryAlphaGovernanceReadiness({
      baseUrl: "http://127.0.0.1:8080",
      candidateId: "candidate::alpha-1",
      alphaId: "alpha::mean-reversion",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.status, 503);
      assert.equal(
        error.errorCode,
        "alpha_lifecycle_action_dependency_unavailable",
      );
      return true;
    },
  );
});

test("Story 6.8 API fails closed when lifecycle-action payload alpha_id mismatches request", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();
  const endpointPayloads = new Map([
    [
      "/control/research/promotion-decisions",
      successEnvelope(
        {
          kind: "decisions",
          candidate_id: "candidate::alpha-1",
          decisions: [],
        },
        "/control/research/promotion-decisions",
      ),
    ],
    [
      "/control/research/shadow-evaluations",
      successEnvelope(
        {
          kind: "evaluations",
          candidate_id: "candidate::alpha-1",
          evaluations: [],
        },
        "/control/research/shadow-evaluations",
      ),
    ],
    [
      "/control/research/validation-runs",
      successEnvelope(
        {
          kind: "runs",
          candidate_id: "candidate::alpha-1",
          runs: [],
        },
        "/control/research/validation-runs",
      ),
    ],
    [
      "/control/research/alpha-health-metrics",
      successEnvelope(
        {
          kind: "metrics",
          alpha_id: "alpha::mean-reversion",
          metrics: [],
        },
        "/control/research/alpha-health-metrics",
      ),
    ],
    [
      "/control/research/alpha-threshold-breaches",
      successEnvelope(
        {
          kind: "breaches",
          alpha_id: "alpha::mean-reversion",
          breaches: [],
        },
        "/control/research/alpha-threshold-breaches",
      ),
    ],
    [
      "/control/research/alpha-lifecycle-actions",
      successEnvelope(
        {
          kind: "actions",
          alpha_id: "alpha::different",
          actions: [],
        },
        "/control/research/alpha-lifecycle-actions",
      ),
    ],
  ]);

  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const payload = endpointPayloads.get(parsed.pathname);
    assert.ok(payload, `Unexpected endpoint request: ${parsed.pathname}`);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  };

  await assert.rejects(
    queryAlphaGovernanceReadiness({
      baseUrl: "http://127.0.0.1:8080",
      candidateId: "candidate::alpha-1",
      alphaId: "alpha::mean-reversion",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.errorCode, "governance_readiness_contract_mismatch");
      return true;
    },
  );
});

test("Story 6.8 API fails closed when lifecycle-action entry alpha_id mismatches request", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();
  const endpointPayloads = new Map([
    [
      "/control/research/promotion-decisions",
      successEnvelope(
        {
          kind: "decisions",
          candidate_id: "candidate::alpha-1",
          decisions: [],
        },
        "/control/research/promotion-decisions",
      ),
    ],
    [
      "/control/research/shadow-evaluations",
      successEnvelope(
        {
          kind: "evaluations",
          candidate_id: "candidate::alpha-1",
          evaluations: [],
        },
        "/control/research/shadow-evaluations",
      ),
    ],
    [
      "/control/research/validation-runs",
      successEnvelope(
        {
          kind: "runs",
          candidate_id: "candidate::alpha-1",
          runs: [],
        },
        "/control/research/validation-runs",
      ),
    ],
    [
      "/control/research/alpha-health-metrics",
      successEnvelope(
        {
          kind: "metrics",
          alpha_id: "alpha::mean-reversion",
          metrics: [],
        },
        "/control/research/alpha-health-metrics",
      ),
    ],
    [
      "/control/research/alpha-threshold-breaches",
      successEnvelope(
        {
          kind: "breaches",
          alpha_id: "alpha::mean-reversion",
          breaches: [],
        },
        "/control/research/alpha-threshold-breaches",
      ),
    ],
    [
      "/control/research/alpha-lifecycle-actions",
      successEnvelope(
        {
          kind: "actions",
          alpha_id: "alpha::mean-reversion",
          actions: [
            {
              action_id: "action-deallocate-001",
              alpha_id: "alpha::different",
              action_type: "deallocate",
              action_status: "applied",
              reason_code: "alpha_lifecycle_action_deallocation_threshold_breached",
              correlation_id: "corr-action-001",
              acted_at_utc: "2026-04-08T09:59:00.000Z",
            },
          ],
        },
        "/control/research/alpha-lifecycle-actions",
      ),
    ],
  ]);

  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    const payload = endpointPayloads.get(parsed.pathname);
    assert.ok(payload, `Unexpected endpoint request: ${parsed.pathname}`);
    return {
      ok: true,
      status: 200,
      json: async () => payload,
    };
  };

  await assert.rejects(
    queryAlphaGovernanceReadiness({
      baseUrl: "http://127.0.0.1:8080",
      candidateId: "candidate::alpha-1",
      alphaId: "alpha::mean-reversion",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.errorCode, "governance_readiness_contract_mismatch");
      return true;
    },
  );
});

test("Story 6.8 API fails closed with governance_readiness_contract_mismatch on malformed envelopes", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();

  const fetchImpl = async (url) => {
    const path = new URL(url).pathname;
    if (path === "/control/research/promotion-decisions") {
      return {
        ok: true,
        status: 200,
        json: async () => ({ data: { kind: "decisions", decisions: {} }, meta: {} }),
      };
    }
    return {
      ok: true,
      status: 200,
      json: async () =>
        successEnvelope(
          { kind: "breaches", alpha_id: "alpha::mean-reversion", breaches: [] },
          path,
        ),
    };
  };

  await assert.rejects(
    queryAlphaGovernanceReadiness({
      baseUrl: "http://127.0.0.1:8080",
      candidateId: "candidate::alpha-1",
      alphaId: "alpha::mean-reversion",
      fetchImpl,
    }),
    (error) => {
      assert.equal(error.errorCode, "governance_readiness_contract_mismatch");
      assert.equal(error.action, "governance_readiness_query");
      return true;
    },
  );
});

test("Story 6.8 API wraps transport failures with machine-readable governance_readiness_request_failed errors", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();

  await assert.rejects(
    queryAlphaGovernanceReadiness({
      baseUrl: "http://127.0.0.1:8080",
      candidateId: "candidate::alpha-1",
      alphaId: "alpha::mean-reversion",
      fetchImpl: async () => {
        throw new Error("connection reset by peer");
      },
    }),
    (error) => {
      assert.equal(error.errorCode, "governance_readiness_request_failed");
      assert.equal(error.action, "governance_readiness_query");
      assert.equal(error.endpoint, "/control/research");
      return true;
    },
  );
});

test("Story 6.8 API rejects non-canonical identifiers before dispatch", async () => {
  const { queryAlphaGovernanceReadiness } = loadCompiledModule();

  await assert.rejects(
    queryAlphaGovernanceReadiness({
      baseUrl: "http://127.0.0.1:8080",
      candidateId: "invalid id",
      alphaId: "alpha::mean-reversion",
      fetchImpl: async () => {
        throw new Error("fetch should not run for invalid identifiers");
      },
    }),
    (error) => {
      assert.equal(error.status, 400);
      assert.equal(error.errorCode, "governance_readiness_invalid_payload");
      assert.equal(error.fieldErrors[0].field, "candidate_id");
      return true;
    },
  );
});
