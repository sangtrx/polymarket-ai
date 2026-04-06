import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.8 control API exposes authenticated rehearsal execute/query routes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/recovery\/rehearsals/);
  assert.match(routes, /\/control\/recovery\/rehearsals\/\{run_id\}/);
  assert.match(routes, /execute_restore_rehearsal/);
  assert.match(routes, /query_restore_rehearsal_by_run_id/);
  assert.match(routes, /query_restore_rehearsals/);
  assert.match(routes, /RecoveryRehearsalDecisionResponse/);
});

test("Story 3.8 governance recovery orchestration enforces rehearsal evidence and severe resume gating", () => {
  const recoveryService = read("services/governance-service/src/recovery/mod.rs");

  assert.match(recoveryService, /execute_restore_rehearsal/);
  assert.match(recoveryService, /deterministic_replay/);
  assert.match(
    recoveryService,
    /recovery_rehearsal_missing_or_failed|RehearsalMissingOrFailed/,
  );
  assert.match(recoveryService, /incident_severity/);
  assert.match(recoveryService, /load_latest_successful_rehearsal_by_correlation/);
});

test("Story 3.8 migration scope is isolated to restore_rehearsal_runs and backup_integrity_checks", () => {
  const migration = read(
    "crates/persistence/migrations/20260406234500_restore_rehearsal_runs.sql",
  );

  assert.match(migration, /CREATE TABLE IF NOT EXISTS restore_rehearsal_runs/);
  assert.match(migration, /CREATE TABLE IF NOT EXISTS backup_integrity_checks/);
  assert.match(migration, /idx_restore_rehearsal_runs_artifact_time/);
  assert.match(migration, /idx_restore_rehearsal_runs_correlation_time/);
  assert.doesNotMatch(migration, /CREATE TABLE IF NOT EXISTS recovery_gate_runs/);
});

test("Story 3.8 domain contracts enforce deterministic signature + strict checksum boundaries", () => {
  const domainModule = read("crates/domain/src/recovery_rehearsal.rs");

  assert.match(domainModule, /canonicalize_restore_output/);
  assert.match(domainModule, /sorted\.sort_by/);
  assert.match(domainModule, /format_ratio_decimal/);
  assert.match(domainModule, /validate_strict_checksum_digest/);
  assert.match(
    domainModule,
    /rehearsal_signature_contract_error|RehearsalSignatureContractError/,
  );
});

test("Story 3.8 operator console safety rail surfaces rehearsal evidence and deterministic replay state", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");
  const controlActions = read("apps/operator-console/src/lib/risk/control-actions.ts");

  assert.match(rail, /queryRestoreRehearsals/);
  assert.match(rail, /queryRestoreRehearsalByRunId/);
  assert.match(rail, /Restore rehearsal evidence/);
  assert.match(rail, /Deterministic replay/);
  assert.match(controlActions, /invokeRestoreRehearsal/);
  assert.match(controlActions, /queryRestoreRehearsals/);
  assert.match(controlActions, /queryRestoreRehearsalByRunId/);
});

test("Story 3.8 runbooks include rehearsal operations and cross-links to recovery + incident + alerts", () => {
  const rehearsalRunbook = read(
    "docs/operations/backup-integrity-restore-rehearsal.md",
  );
  const recoveryRunbook = read(
    "docs/operations/controlled-recovery-readiness-gates.md",
  );
  const incidentRunbook = read(
    "docs/operations/incident-search-causal-timeline-forensics.md",
  );
  const alertsRunbook = read("docs/operations/severity-alert-delivery.md");

  assert.match(rehearsalRunbook, /\/control\/recovery\/rehearsals/);
  assert.match(rehearsalRunbook, /recovery_rehearsal_missing_or_failed/);
  assert.match(
    recoveryRunbook,
    /backup-integrity-restore-rehearsal\.md/,
  );
  assert.match(
    incidentRunbook,
    /backup-integrity-restore-rehearsal\.md/,
  );
  assert.match(alertsRunbook, /backup-integrity-restore-rehearsal\.md/);
});
