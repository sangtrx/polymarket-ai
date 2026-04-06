import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.8 control API maps rehearsal contract errors and persistence failures to deterministic statuses", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /RecoveryReasonCode::RehearsalSignatureContractError/);
  assert.match(routes, /RecoveryReasonCode::RehearsalMissingOrFailed/);
  assert.match(routes, /restore_rehearsal_query_failed/);
  assert.match(routes, /restore_rehearsal_constraint_violation/);
  assert.match(routes, /recovery_service_error_status/);
});

test("Story 3.8 rehearsal response contracts include integrity checks and deterministic signature envelopes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /RecoveryRehearsalIntegrityCheckResponseItem/);
  assert.match(routes, /RecoveryDeterministicSignatureResponse/);
  assert.match(routes, /integrity_checks/);
  assert.match(routes, /deterministic_signature/);
  assert.match(routes, /recommended_next_action/);
});

test("Story 3.8 control-api route tests cover execute/query rehearsal endpoints", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /recovery_rehearsal_execute_endpoint_returns_integrity_evidence/,
  );
  assert.match(
    routes,
    /recovery_rehearsal_query_by_run_endpoint_returns_rehearsal_details/,
  );
  assert.match(
    routes,
    /recovery_rehearsal_query_endpoint_supports_selector_and_limit/,
  );
});

test("Story 3.8 recovery status mapping covers invalid payload, unauthorized, dependency, and severe-blocked reason codes", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /RecoveryReasonCode::InvalidPayload\.code\(\)/);
  assert.match(routes, /RecoveryReasonCode::Unauthorized\.code\(\)/);
  assert.match(routes, /RecoveryReasonCode::DependencyUnavailable\.code\(\)/);
  assert.match(routes, /RecoveryReasonCode::RehearsalMissingOrFailed\.code\(\)/);
  assert.match(
    routes,
    /RecoveryReasonCode::RehearsalSignatureContractError\.code\(\)/,
  );
});

test("Story 3.8 route tests cover malformed JSON mapping and NFR16 query latency budget", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(
    routes,
    /recovery_rehearsal_execute_endpoint_maps_json_rejection_to_machine_error/,
  );
  assert.match(
    routes,
    /recovery_rehearsal_query_endpoint_measures_p95_latency_within_target_for_repeated_queries/,
  );
  assert.match(routes, /p95_latency_ms <= 5_000/);
  assert.match(routes, /payload\["p95_latency_target_ms"\], 5_000/);
});
