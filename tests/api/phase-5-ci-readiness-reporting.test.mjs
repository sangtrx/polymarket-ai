import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("RPTG-01: control-api wires authenticated readiness report routes under report-exports", () => {
  const routes = read("services/control-api/src/routes/mod.rs");

  assert.match(routes, /\/control\/report-exports\/readiness\/on-demand/);
  assert.match(routes, /\/control\/report-exports\/readiness\/\{job_id\}/);
  assert.match(
    routes,
    /report_export_routes[\s\S]*route_layer\(axum_middleware::from_fn_with_state\([\s\S]*require_authenticated_actor/,
  );
});

test("RPTG-02: readiness markdown composer includes required operator summary sections", () => {
  const readiness = read("services/reporting-service/src/exports/readiness.rs");

  assert.match(readiness, /## Coverage Posture/);
  assert.match(readiness, /## Top Unresolved Risks/);
  assert.match(readiness, /## Waiver Ledger/);
  assert.match(readiness, /## Recommendation Rationale/);
});

test("RPTG-01/RPTG-02: phase-5 aggregate QA script keeps portable cargo resolution and required checks", () => {
  const pkg = JSON.parse(read("package.json"));
  const cargoRunner = pkg.scripts["qa:test:phase:cargo"];
  const phase5 = pkg.scripts["qa:test:phase-5"];

  assert.match(cargoRunner, /CARGO_BIN=/);
  assert.match(cargoRunner, /\.cargo\/bin\/cargo/);
  assert.match(
    phase5,
    /qa:test:phase:cargo -- test -p research-gateway readiness::tests::/,
  );
  assert.match(
    phase5,
    /qa:test:phase:cargo -- test -p persistence postgres::readiness::tests::waiver_/,
  );
  assert.match(
    phase5,
    /qa:test:phase:cargo -- test -p reporting-service exports::workflows::tests::readiness_/,
  );
  assert.match(
    phase5,
    /qa:test:phase:cargo -- test -p research-gateway main::tests::readiness_/,
  );
  assert.match(
    phase5,
    /node --test tests\/api\/phase-5-ci-readiness-reporting\.test\.mjs tests\/e2e\/phase-5-ci-readiness-reporting\.e2e\.test\.mjs/,
  );
});
