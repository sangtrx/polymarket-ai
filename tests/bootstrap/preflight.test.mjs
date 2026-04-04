import test from "node:test";
import assert from "node:assert/strict";
import { BootstrapError, compareSemver, parseSemver } from "../../tools/bootstrap/check-utils.mjs";
import {
  parseRustToolchainChannel,
  validateNodeVersion,
  validatePnpmVersion,
  validateRustVersion,
} from "../../tools/bootstrap/preflight.mjs";

test("parseSemver normalizes v-prefixed version strings", () => {
  assert.deepEqual(parseSemver("v22.20.0"), [22, 20, 0]);
});

test("compareSemver honors semantic ordering", () => {
  assert.equal(compareSemver([20, 9, 0], [20, 8, 9]), 1);
  assert.equal(compareSemver([20, 9, 0], [20, 9, 0]), 0);
  assert.equal(compareSemver([20, 8, 9], [20, 9, 0]), -1);
});

test("validateNodeVersion enforces Node >= 20.9.0", () => {
  assert.doesNotThrow(() => validateNodeVersion("v20.9.0"));
  assert.throws(
    () => validateNodeVersion("v20.8.9"),
    (error) =>
      error instanceof BootstrapError &&
      error.payload.error_code === "BOOTSTRAP_TOOLCHAIN_MISMATCH",
  );
});

test("validatePnpmVersion enforces pnpm major version 10", () => {
  assert.doesNotThrow(() => validatePnpmVersion("10.10.0"));
  assert.throws(
    () => validatePnpmVersion("9.15.4"),
    (error) =>
      error instanceof BootstrapError &&
      error.payload.error_code === "BOOTSTRAP_TOOLCHAIN_MISMATCH",
  );
});

test("validateRustVersion requires exact toolchain pin match", () => {
  assert.doesNotThrow(() => validateRustVersion("1.90.0", "1.90.0", "rustc"));
  assert.throws(
    () => validateRustVersion("1.90.0", "1.89.1", "rustc"),
    (error) =>
      error instanceof BootstrapError &&
      error.payload.failed_check === "rustc-version",
  );
});

test("parseRustToolchainChannel extracts channel from file content", () => {
  const channel = parseRustToolchainChannel('[toolchain]\nchannel = "1.90.0"\n');
  assert.equal(channel, "1.90.0");
});
