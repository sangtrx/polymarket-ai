import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

function read(relativePath) {
  return readFileSync(resolve(relativePath), "utf8");
}

test("Story 3.2 root QA command wires web quality gates and story-scoped suites", () => {
  const packageJson = read("package.json");

  assert.match(packageJson, /"qa:test:story-3-2"/);
  assert.match(packageJson, /npm run web:lint/);
  assert.match(packageJson, /npm run web:typecheck/);
  assert.match(packageJson, /npm run web:build/);
  assert.match(packageJson, /tests\/story-3-2\/\*\.test\.mjs/);
  assert.match(packageJson, /tests\/api\/story-3-2\*\.test\.mjs/);
  assert.match(packageJson, /tests\/e2e\/story-3-2\*\.test\.mjs/);
});

test("Story 3.2 control API client keeps canonical emergency endpoint contract", () => {
  const controlClient = read("apps/operator-console/src/lib/risk/control-actions.ts");
  const env = read("apps/operator-console/src/lib/env.ts");

  assert.match(controlClient, /\/control\/emergency\/pause/);
  assert.match(controlClient, /\/control\/emergency\/reduce-only/);
  assert.match(controlClient, /\/control\/emergency\/cancel-all/);
  assert.match(
    controlClient,
    /\/control\/emergency\/actions\/\$\{encodeURIComponent\(actionId\)\}/,
  );
  assert.match(controlClient, /error_code/);
  assert.match(controlClient, /reason_code/);
  assert.match(controlClient, /resulting_mode/);
  assert.match(controlClient, /timestamp_utc/);
  assert.match(controlClient, /audit_reference/);
  assert.match(env, /NEXT_PUBLIC_OPERATOR_CONSOLE_API_BASE_URL/);
});

test("Story 3.2 top bar and route copy remove temporary non-interactive destructive-control placeholders", () => {
  const topBar = read("apps/operator-console/src/components/shell/ShellTopBar.tsx");
  const incidents = read("apps/operator-console/src/app/(incidents)/incidents/page.tsx");
  const governance = read("apps/operator-console/src/app/(governance)/governance/page.tsx");

  assert.doesNotMatch(topBar, /Destructive controls disabled by default/);
  assert.doesNotMatch(incidents, /Destructive controls remain disabled until Story 3\.2/);
  assert.doesNotMatch(governance, /No mutation actions in Story 3\.1/);
});

test("Story 3.2 e2e safety rail renders timestamped confirmation and machine-readable error evidence", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /Post-action confirmation/);
  assert.match(rail, /Action ID/);
  assert.match(rail, /Correlation ID/);
  assert.match(rail, /Audit reference/);
  assert.match(rail, /Error code/);
  assert.match(rail, /safety-action-evidence" role="status"/);
  assert.match(rail, /safety-action-error" role="alert"/);
});

test("Story 3.2 e2e danger-action path enforces explicit confirmation dialog semantics", () => {
  const rail = read("apps/operator-console/src/components/risk/SafetyActionRail.tsx");

  assert.match(rail, /role="alertdialog"/);
  assert.match(rail, /aria-live="assertive"/);
  assert.match(rail, /requires explicit operator intent confirmation/i);
  assert.match(rail, /Confirm action/);
  assert.match(rail, /Cancel/);
});
