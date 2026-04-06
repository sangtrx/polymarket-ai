import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

const schemaFiles = [
  "services/reporting-service/src/contracts/artifacts/v1/trades.schema.json",
  "services/reporting-service/src/contracts/artifacts/v1/positions.schema.json",
  "services/reporting-service/src/contracts/artifacts/v1/risk-events.schema.json",
  "services/reporting-service/src/contracts/artifacts/v1/performance.schema.json",
  "services/reporting-service/src/contracts/artifacts/v1/alpha-attribution.schema.json",
];

test("Story 4.2 schema artifacts are published for each reporting dataset contract", () => {
  for (const file of schemaFiles) {
    const parsed = JSON.parse(read(file));
    assert.equal(parsed.type, "object");
    assert.ok(Array.isArray(parsed.required));
    assert.ok(parsed.required.includes("data"));
    assert.ok(parsed.required.includes("meta"));
    assert.ok(parsed.required.includes("error"));

    const metaRequired = parsed?.$defs?.meta?.required;
    assert.ok(Array.isArray(metaRequired), `${file} must define $defs.meta.required`);
    assert.ok(metaRequired.includes("contract_version"));
    assert.ok(metaRequired.includes("reason_code"));
    assert.ok(metaRequired.includes("correlation_id"));
  }
});

test("Story 4.2 changelog artifact is discoverable with lifecycle policy metadata", () => {
  const changelog = JSON.parse(
    read("services/reporting-service/src/contracts/artifacts/v1/changelog.json"),
  );
  assert.equal(changelog.contract_version, "v1");
  assert.equal(changelog.support_policy.minimum_deprecation_notice_days, 90);
  assert.equal(
    changelog.support_policy.minimum_backward_compatibility_days_after_replacement,
    180,
  );
  assert.ok(Array.isArray(changelog.entries));
  assert.ok(changelog.entries.some((entry) => entry.dataset === "trades"));
  assert.ok(changelog.entries.some((entry) => entry.dataset === "positions"));
  assert.ok(changelog.entries.some((entry) => entry.dataset === "risk-events"));
  assert.ok(changelog.entries.some((entry) => entry.dataset === "performance"));
  assert.ok(changelog.entries.some((entry) => entry.dataset === "alpha-attribution"));
});

test("Story 4.2 reporting-service exposes version metadata and artifact discovery routes", () => {
  const apiSource = read("services/reporting-service/src/api.rs");
  assert.match(apiSource, /\/api\/v1\/reporting\/contracts\/\{dataset\}\/versions/);
  assert.match(apiSource, /\/api\/v1\/reporting\/contracts\/\{dataset\}\/versions\/\{contract_version\}/);
  assert.match(
    apiSource,
    /\/api\/v1\/reporting\/contracts\/\{dataset\}\/versions\/\{contract_version\}\/schema/,
  );
  assert.match(
    apiSource,
    /\/api\/v1\/reporting\/contracts\/\{dataset\}\/versions\/\{contract_version\}\/changelog/,
  );
});
