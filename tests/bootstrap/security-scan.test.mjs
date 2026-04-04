import { cpSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import assert from "node:assert/strict";
import { BootstrapError } from "../../tools/bootstrap/check-utils.mjs";
import { runSecurityScan } from "../../tools/bootstrap/security-scan.mjs";

const FIXTURE_ROOT = resolve("tests/fixtures/valid");

function withFixtureCopy(mutator) {
  const tempRoot = mkdtempSync(join(tmpdir(), "security-scan-"));
  cpSync(FIXTURE_ROOT, tempRoot, { recursive: true });
  writeFileSync(resolve(tempRoot, "pnpm-lock.yaml"), "lockfileVersion: '9.0'\n", "utf8");
  mutator(tempRoot);
  return tempRoot;
}

test("runSecurityScan passes for valid fixture", () => {
  const fixtureRoot = withFixtureCopy(() => {});
  try {
    const summary = runSecurityScan(fixtureRoot);
    assert.deepEqual(
      summary.check_matrix.map((item) => item.check),
      ["dependency_lockfiles", "secret_material_scan"],
    );
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("runSecurityScan rejects private key literals in repository source files", () => {
  const fixtureRoot = withFixtureCopy((tempRoot) => {
    const injectedPath = resolve(tempRoot, "apps/operator-console/leaky-key.txt");
    const privateKeyLiteral =
      "-----BEGIN " +
      "PRIVATE KEY-----\n" +
      "leaky-material\n" +
      "-----END PRIVATE KEY-----\n";
    writeFileSync(injectedPath, privateKeyLiteral, "utf8");
  });

  try {
    assert.throws(
      () => runSecurityScan(fixtureRoot),
      (error) =>
        error instanceof BootstrapError &&
        error.payload.error_code === "SECURITY_PRIVATE_KEY_LITERAL" &&
        error.payload.failed_check === "secret-material-scan",
    );
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("runSecurityScan rejects sensitive CLI arguments with literal secret values", () => {
  const fixtureRoot = withFixtureCopy((tempRoot) => {
    const injectedPath = resolve(tempRoot, "apps/operator-console/leaky-args.sh");
    const literalSecretArg = "--api" + "-key=hardcoded-live-secret";
    writeFileSync(injectedPath, `curl ${literalSecretArg} https://example.test\n`, "utf8");
  });

  try {
    assert.throws(
      () => runSecurityScan(fixtureRoot),
      (error) =>
        error instanceof BootstrapError &&
        error.payload.error_code === "SECURITY_SENSITIVE_CLI_ARG" &&
        error.payload.failed_check === "sensitive-cli-args",
    );
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
